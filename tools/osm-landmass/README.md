# osm-landmass

Generate clean landmass polygons from OpenStreetMap PBF data by extracting coastlines and subtracting inland water bodies.

## Overview

OSM's coastline-derived "land polygons" incorrectly treat rivers and lakes as land. This tool:

1. Extracts coastline ways (`natural=coastline`) and optionally tidal features (`tidal=yes`)
2. Assembles them into closed land polygons
3. Extracts inland water bodies (`natural=water`, `waterway=riverbank`, `landuse=reservoir`)
4. Subtracts water from land to produce true landmass shapes

## Installation

```bash
cd tools/osm-landmass
cargo build --release
```

## Usage

### Basic usage

```bash
osm-landmass -i nyc.osm.pbf -o landmass.geojson
```

### With debug outputs

```bash
osm-landmass -i nyc.osm.pbf -o landmass.geojson \
  --land-output land.geojson \
  --water-output water.geojson \
  --verbose
```

### Skip water subtraction (raw coastline output)

```bash
osm-landmass -i nyc.osm.pbf -o coastline.geojson --skip-water
```

## Options

| Option | Description |
|--------|-------------|
| `-i, --input <FILE>` | Input OSM PBF file (required) |
| `-o, --output <FILE>` | Output GeoJSON file for landmass polygons (required) |
| `--land-output <FILE>` | Also output raw land polygons before water subtraction |
| `--water-output <FILE>` | Also output water polygons |
| `--include-tidal` | Include `tidal=yes` features in land area (default: true) |
| `--skip-water` | Skip water body subtraction |
| `-v, --verbose` | Verbose output (show debug messages) |

## Output Format

The output is a GeoJSON FeatureCollection where each landmass polygon is a separate Feature with properties:

```json
{
  "type": "Feature",
  "geometry": { "type": "Polygon", ... },
  "properties": {
    "feature_type": "landmass",
    "area_sqm": 12345678.9,
    "index": 0
  }
}
```

## OSM Tags Processed

### Land features (assembled into land polygons)
- `natural=coastline` - Coastline ways
- `tidal=yes` - Tidal features (when `--include-tidal` is set)

### Water features (subtracted from land)
- `natural=water` - Lakes, ponds, basins
- `waterway=riverbank` - Wide rivers (Hudson, Harlem, etc.)
- `landuse=reservoir` - Reservoirs
- `water=*` - Various water body types

### Multipolygon relations
The tool handles OSM multipolygon relations with `outer` and `inner` roles for complex water bodies with islands.

## Example: NYC

For the NYC region, the tool will:
- Create proper separation between Manhattan and the Bronx (Harlem River)
- Subtract the Hudson River from the west side
- Create holes for Central Park Reservoir and other inland water

## Technical Notes

### Ring Assembly
Coastline ways in OSM are stored as connected segments that need to be joined at shared endpoints. The tool:
1. Builds an endpoint index for efficient matching
2. Connects ways that share endpoints (handling reversed ways)
3. Tracks open rings that cross the PBF boundary (warns but skips)

### Performance
- Uses `hashbrown` for faster HashMap operations
- Single-pass PBF reading with element filtering
- Progress reporting during long operations
- Typical: 2-5 minutes for ~100MB PBF file

### Edge Cases
- Open rings (crossing PBF boundary): Logged as warnings, skipped
- Invalid geometries: Filtered with warnings
- Self-intersecting polygons: Attempted repair via union trick
