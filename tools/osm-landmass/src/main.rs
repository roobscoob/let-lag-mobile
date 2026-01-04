use anyhow::{bail, Context, Result};
use clap::Parser;
use geo::MultiPolygon;
use std::path::PathBuf;

mod assembly;
mod clip;
mod geometry;
mod output;
mod pbf;

use assembly::{assemble_multipolygon, assemble_rings, build_way_index, closed_ways_to_polygons, rings_to_simple_polygons};
use clip::{clip_to_boundary, close_rings_against_boundary, read_clip_polygon};
use geometry::{calculate_area_sqm, compute_landmass, filter_valid_polygons, union_all, GeometryStats};
use output::{write_landmass_geojson, write_multipolygon_geojson};
use pbf::{extract_elements, resolve_ways};

#[derive(Parser, Debug)]
#[command(
    name = "osm-landmass",
    author,
    version,
    about = "Generate landmass polygons from OpenStreetMap PBF data",
    long_about = "Extracts coastlines from OSM PBF files, subtracts inland water bodies, \
                  and outputs clean landmass polygons as GeoJSON.\n\n\
                  The tool assembles land areas from natural=coastline ways and optionally \
                  tidal=yes features, then subtracts water bodies (natural=water, \
                  waterway=riverbank, landuse=reservoir) to produce true landmass shapes."
)]
struct Args {
    /// Input OSM PBF file
    #[arg(short, long)]
    input: PathBuf,

    /// Output GeoJSON file for landmass polygons
    #[arg(short, long)]
    output: PathBuf,

    /// Also output raw land polygons (before water subtraction) to this file
    #[arg(long)]
    land_output: Option<PathBuf>,

    /// Also output water polygons to this file
    #[arg(long)]
    water_output: Option<PathBuf>,

    /// Include tidal=yes features in land area
    #[arg(long, default_value = "true")]
    include_tidal: bool,

    /// Skip water body subtraction (output raw coastline polygons)
    #[arg(long)]
    skip_water: bool,

    /// Verbose output (show debug messages)
    #[arg(short, long)]
    verbose: bool,

    /// Clip polygon GeoJSON file (used to close coastlines at boundary)
    /// Open coastlines that cross the boundary will be closed by walking
    /// along the clip polygon edge. The area outside the clip is treated as water.
    #[arg(short, long)]
    clip: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(if args.verbose { "debug" } else { "info" }),
    )
    .format_timestamp(None)
    .init();

    log::info!("=== OSM Landmass Generator ===");
    log::info!("Input: {}", args.input.display());
    log::info!("Output: {}", args.output.display());

    // Validate input file exists
    if !args.input.exists() {
        bail!("Input file does not exist: {}", args.input.display());
    }

    // Load clip polygon if provided
    let clip_polygon = if let Some(clip_path) = &args.clip {
        log::info!("Clip polygon: {}", clip_path.display());
        Some(read_clip_polygon(clip_path).context("Failed to read clip polygon")?)
    } else {
        None
    };

    let mut stats = GeometryStats::default();

    // Phase 1: Extract elements from PBF
    log::info!("");
    log::info!("Phase 1: Parsing PBF file...");
    let elements = extract_elements(&args.input, args.include_tidal)
        .context("Failed to extract elements from PBF")?;

    // Phase 2: Build land polygons from coastlines
    log::info!("");
    log::info!("Phase 2: Building land polygons from coastlines...");

    let coastline_ways = resolve_ways(&elements.coastline_ways, &elements.nodes);
    log::info!("  Resolved {} coastline ways", coastline_ways.len());

    let coastline_rings = assemble_rings(coastline_ways);
    log::info!(
        "  Assembled {} closed rings, {} open rings",
        coastline_rings.closed_rings.len(),
        coastline_rings.open_rings.len()
    );

    let mut land_polygons = rings_to_simple_polygons(coastline_rings.closed_rings);
    log::info!("  Created {} land polygons from closed coastlines", land_polygons.len());

    // Handle open rings - close them against clip boundary if provided
    if !coastline_rings.open_rings.is_empty() {
        if let Some(ref clip_poly) = clip_polygon {
            log::info!(
                "  Closing {} open coastline rings against clip boundary...",
                coastline_rings.open_rings.len()
            );
            let closed_from_boundary = close_rings_against_boundary(coastline_rings.open_rings, clip_poly);
            log::info!("    Created {} polygons from boundary-closed rings", closed_from_boundary.len());
            land_polygons.extend(closed_from_boundary);
        } else {
            log::warn!(
                "  {} coastline rings could not be closed (may cross PBF boundary)",
                coastline_rings.open_rings.len()
            );
            log::warn!("    Hint: Use --clip <geojson> to provide a clip polygon for closing these rings");
        }
    }

    // Add tidal features if enabled
    if args.include_tidal && !elements.tidal_ways.is_empty() {
        log::info!("  Adding tidal features...");
        let tidal_ways = resolve_ways(&elements.tidal_ways, &elements.nodes);
        let tidal_polygons = closed_ways_to_polygons(&tidal_ways);
        log::info!("    Added {} tidal polygons", tidal_polygons.len());
        land_polygons.extend(tidal_polygons);
    }

    // Process land multipolygon relations
    if !elements.land_relations.is_empty() {
        log::info!("  Processing {} land relations...", elements.land_relations.len());
        let way_index = build_way_index(&elements.coastline_ways, &elements.nodes);

        for rel in &elements.land_relations {
            if let Some(mp) = assemble_multipolygon(rel, &way_index) {
                land_polygons.extend(mp.0);
            }
        }
    }

    // Filter invalid polygons
    land_polygons = filter_valid_polygons(land_polygons);
    stats.land_polygon_count = land_polygons.len();

    if land_polygons.is_empty() {
        bail!("No valid land polygons found. Is this an inland area without coastlines?");
    }

    // Union all land polygons
    log::info!("  Unioning {} land polygons...", land_polygons.len());
    let land = union_all(land_polygons);
    stats.land_area_sqm = calculate_area_sqm(&land);
    log::info!("  Land area: {:.2} km²", stats.land_area_sqm / 1_000_000.0);

    // Optional: Write raw land output
    if let Some(land_path) = &args.land_output {
        write_multipolygon_geojson(&land, land_path, "land")
            .context("Failed to write land GeoJSON")?;
        log::info!("  Wrote raw land to {}", land_path.display());
    }

    // Phase 3: Build water polygons
    log::info!("");
    log::info!("Phase 3: Building water polygons...");

    let mut water_polygons = Vec::new();

    // Process simple water ways (closed polygons)
    let water_ways = resolve_ways(&elements.water_ways, &elements.nodes);
    let simple_water = closed_ways_to_polygons(&water_ways);
    log::info!("  {} water polygons from closed ways", simple_water.len());
    water_polygons.extend(simple_water);

    // Process water multipolygon relations
    if !elements.water_relations.is_empty() {
        log::info!(
            "  Processing {} water relations...",
            elements.water_relations.len()
        );

        // Build index of all water ways
        let all_water_refs: Vec<_> = elements
            .water_ways
            .iter()
            .chain(elements.coastline_ways.iter())
            .cloned()
            .collect();
        let water_way_index = build_way_index(&all_water_refs, &elements.nodes);

        for rel in &elements.water_relations {
            if let Some(mp) = assemble_multipolygon(rel, &water_way_index) {
                water_polygons.extend(mp.0);
            }
        }
    }

    // Filter invalid water polygons
    water_polygons = filter_valid_polygons(water_polygons);
    stats.water_polygon_count = water_polygons.len();

    log::info!("  Total: {} valid water polygons", water_polygons.len());

    // Union water polygons
    let water = if water_polygons.is_empty() {
        MultiPolygon::new(vec![])
    } else {
        log::info!("  Unioning water polygons...");
        union_all(water_polygons)
    };
    stats.water_area_sqm = calculate_area_sqm(&water);
    log::info!("  Water area: {:.2} km²", stats.water_area_sqm / 1_000_000.0);

    // Optional: Write water output
    if let Some(water_path) = &args.water_output {
        write_multipolygon_geojson(&water, water_path, "water")
            .context("Failed to write water GeoJSON")?;
        log::info!("  Wrote water to {}", water_path.display());
    }

    // Phase 4: Compute landmass (land - water)
    log::info!("");
    let mut landmass = if args.skip_water {
        log::info!("Phase 4: Skipping water subtraction (--skip-water)");
        land
    } else {
        log::info!("Phase 4: Computing landmass (land - water)...");
        compute_landmass(land, water)
    };

    // Clip to boundary if provided (ensures nothing extends outside)
    if let Some(ref clip_poly) = clip_polygon {
        log::info!("  Clipping landmass to boundary...");
        let before_count = landmass.0.len();
        landmass = clip_to_boundary(landmass, clip_poly);
        log::info!("    {} polygons -> {} polygons after clipping", before_count, landmass.0.len());
    }

    stats.landmass_polygon_count = landmass.0.len();
    stats.landmass_area_sqm = calculate_area_sqm(&landmass);

    // Phase 5: Write output
    log::info!("");
    log::info!("Phase 5: Writing output...");
    write_landmass_geojson(&landmass, &args.output)
        .context("Failed to write landmass GeoJSON")?;

    // Summary
    log::info!("");
    stats.log_summary();
    log::info!("");
    log::info!("Output written to: {}", args.output.display());
    log::info!("Done!");

    Ok(())
}
