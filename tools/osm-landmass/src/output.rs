use anyhow::{Context, Result};
use geo::algorithm::area::Area;
use geo::{MultiPolygon, Polygon};
use geojson::{Feature, FeatureCollection, GeoJson, Geometry, Value};
use std::path::Path;

/// Convert a geo Polygon to GeoJSON Value
fn polygon_to_geojson(poly: &Polygon<f64>) -> Value {
    let exterior: Vec<Vec<f64>> = poly
        .exterior()
        .0
        .iter()
        .map(|c| vec![c.x, c.y])
        .collect();

    let mut rings = vec![exterior];

    for interior in poly.interiors() {
        let hole: Vec<Vec<f64>> = interior.0.iter().map(|c| vec![c.x, c.y]).collect();
        rings.push(hole);
    }

    Value::Polygon(rings)
}

/// Convert a geo MultiPolygon to GeoJSON Value
fn multipolygon_to_geojson(mp: &MultiPolygon<f64>) -> Value {
    let polygons: Vec<Vec<Vec<Vec<f64>>>> = mp
        .0
        .iter()
        .map(|poly| {
            let exterior: Vec<Vec<f64>> =
                poly.exterior().0.iter().map(|c| vec![c.x, c.y]).collect();

            let mut rings = vec![exterior];

            for interior in poly.interiors() {
                let hole: Vec<Vec<f64>> = interior.0.iter().map(|c| vec![c.x, c.y]).collect();
                rings.push(hole);
            }

            rings
        })
        .collect();

    Value::MultiPolygon(polygons)
}

/// Create a GeoJSON Feature from a polygon with properties
fn polygon_to_feature(
    poly: &Polygon<f64>,
    feature_type: &str,
    index: usize,
) -> Feature {
    let area_sqm = poly.unsigned_area() * 111_320.0 * 85_000.0; // Approximate

    let mut properties = serde_json::Map::new();
    properties.insert("feature_type".to_string(), serde_json::json!(feature_type));
    properties.insert("area_sqm".to_string(), serde_json::json!(area_sqm));
    properties.insert("index".to_string(), serde_json::json!(index));

    Feature {
        bbox: None,
        geometry: Some(Geometry::new(polygon_to_geojson(poly))),
        id: None,
        properties: Some(properties),
        foreign_members: None,
    }
}

/// Create a GeoJSON Feature from a MultiPolygon
fn multipolygon_to_feature(
    mp: &MultiPolygon<f64>,
    feature_type: &str,
) -> Feature {
    let area_sqm = mp.unsigned_area() * 111_320.0 * 85_000.0; // Approximate

    let mut properties = serde_json::Map::new();
    properties.insert("feature_type".to_string(), serde_json::json!(feature_type));
    properties.insert("area_sqm".to_string(), serde_json::json!(area_sqm));
    properties.insert("polygon_count".to_string(), serde_json::json!(mp.0.len()));

    let hole_count: usize = mp.0.iter().map(|p| p.interiors().len()).sum();
    properties.insert("hole_count".to_string(), serde_json::json!(hole_count));

    Feature {
        bbox: None,
        geometry: Some(Geometry::new(multipolygon_to_geojson(mp))),
        id: None,
        properties: Some(properties),
        foreign_members: None,
    }
}

/// Write polygons to a GeoJSON file (each polygon as separate feature)
pub fn write_polygons_geojson(
    polygons: &[Polygon<f64>],
    output_path: &Path,
    feature_type: &str,
) -> Result<()> {
    log::info!("Writing {} polygons to {}", polygons.len(), output_path.display());

    let features: Vec<Feature> = polygons
        .iter()
        .enumerate()
        .map(|(i, poly)| polygon_to_feature(poly, feature_type, i))
        .collect();

    let feature_collection = FeatureCollection {
        bbox: None,
        features,
        foreign_members: None,
    };

    let geojson = GeoJson::from(feature_collection);
    let json_string = serde_json::to_string_pretty(&geojson)
        .context("Failed to serialize GeoJSON")?;

    std::fs::write(output_path, json_string)
        .with_context(|| format!("Failed to write GeoJSON to {}", output_path.display()))?;

    Ok(())
}

/// Write a MultiPolygon to a GeoJSON file (as single feature)
pub fn write_multipolygon_geojson(
    mp: &MultiPolygon<f64>,
    output_path: &Path,
    feature_type: &str,
) -> Result<()> {
    log::info!(
        "Writing MultiPolygon ({} polygons) to {}",
        mp.0.len(),
        output_path.display()
    );

    let feature = multipolygon_to_feature(mp, feature_type);

    let feature_collection = FeatureCollection {
        bbox: None,
        features: vec![feature],
        foreign_members: None,
    };

    let geojson = GeoJson::from(feature_collection);
    let json_string = serde_json::to_string_pretty(&geojson)
        .context("Failed to serialize GeoJSON")?;

    std::fs::write(output_path, json_string)
        .with_context(|| format!("Failed to write GeoJSON to {}", output_path.display()))?;

    Ok(())
}

/// Write landmass result with each polygon as separate feature
pub fn write_landmass_geojson(
    mp: &MultiPolygon<f64>,
    output_path: &Path,
) -> Result<()> {
    log::info!(
        "Writing landmass ({} polygons) to {}",
        mp.0.len(),
        output_path.display()
    );

    let features: Vec<Feature> = mp
        .0
        .iter()
        .enumerate()
        .map(|(i, poly)| polygon_to_feature(poly, "landmass", i))
        .collect();

    let feature_collection = FeatureCollection {
        bbox: None,
        features,
        foreign_members: None,
    };

    let geojson = GeoJson::from(feature_collection);
    let json_string = serde_json::to_string_pretty(&geojson)
        .context("Failed to serialize GeoJSON")?;

    std::fs::write(output_path, json_string)
        .with_context(|| format!("Failed to write GeoJSON to {}", output_path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::{Coord, LineString};

    #[test]
    fn test_polygon_to_geojson() {
        let poly = Polygon::new(
            LineString::new(vec![
                Coord { x: 0.0, y: 0.0 },
                Coord { x: 1.0, y: 0.0 },
                Coord { x: 1.0, y: 1.0 },
                Coord { x: 0.0, y: 1.0 },
                Coord { x: 0.0, y: 0.0 },
            ]),
            vec![],
        );

        let value = polygon_to_geojson(&poly);

        match value {
            Value::Polygon(rings) => {
                assert_eq!(rings.len(), 1);
                assert_eq!(rings[0].len(), 5);
            }
            _ => panic!("Expected Polygon value"),
        }
    }

    #[test]
    fn test_polygon_with_hole() {
        let exterior = LineString::new(vec![
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 10.0, y: 0.0 },
            Coord { x: 10.0, y: 10.0 },
            Coord { x: 0.0, y: 10.0 },
            Coord { x: 0.0, y: 0.0 },
        ]);

        let hole = LineString::new(vec![
            Coord { x: 2.0, y: 2.0 },
            Coord { x: 8.0, y: 2.0 },
            Coord { x: 8.0, y: 8.0 },
            Coord { x: 2.0, y: 8.0 },
            Coord { x: 2.0, y: 2.0 },
        ]);

        let poly = Polygon::new(exterior, vec![hole]);
        let value = polygon_to_geojson(&poly);

        match value {
            Value::Polygon(rings) => {
                assert_eq!(rings.len(), 2); // exterior + 1 hole
            }
            _ => panic!("Expected Polygon value"),
        }
    }
}
