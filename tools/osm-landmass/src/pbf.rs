use anyhow::{Context, Result};
use geo::Coord;
use hashbrown::HashMap;
use indicatif::{ProgressBar, ProgressStyle};
use osmpbf::{Element, ElementReader};
use std::path::Path;

/// Node ID to coordinate mapping
pub struct NodeCache {
    nodes: HashMap<i64, Coord<f64>>,
}

impl NodeCache {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            nodes: HashMap::with_capacity(capacity),
        }
    }

    pub fn insert(&mut self, id: i64, coord: Coord<f64>) {
        self.nodes.insert(id, coord);
    }

    pub fn get(&self, id: i64) -> Option<&Coord<f64>> {
        self.nodes.get(&id)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }
}

/// Tag storage type
pub type Tags = HashMap<String, String>;

/// A way with node references (before coordinate resolution)
#[derive(Clone)]
pub struct WayRef {
    pub id: i64,
    pub node_refs: Vec<i64>,
    pub tags: Tags,
}

impl WayRef {
    pub fn first_node(&self) -> Option<i64> {
        self.node_refs.first().copied()
    }

    pub fn last_node(&self) -> Option<i64> {
        self.node_refs.last().copied()
    }

    pub fn is_closed(&self) -> bool {
        self.node_refs.len() >= 4 && self.first_node() == self.last_node()
    }
}

/// A way with resolved coordinates
#[derive(Clone)]
pub struct Way {
    pub id: i64,
    pub coords: Vec<Coord<f64>>,
    pub tags: Tags,
}

impl Way {
    pub fn first_coord(&self) -> Option<Coord<f64>> {
        self.coords.first().copied()
    }

    pub fn last_coord(&self) -> Option<Coord<f64>> {
        self.coords.last().copied()
    }

    pub fn is_closed(&self) -> bool {
        if self.coords.len() < 4 {
            return false;
        }
        let first = self.coords.first().unwrap();
        let last = self.coords.last().unwrap();
        (first.x - last.x).abs() < 1e-9 && (first.y - last.y).abs() < 1e-9
    }
}

/// Member role in a relation
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MemberRole {
    Outer,
    Inner,
    Unknown,
}

/// Relation member
#[derive(Clone)]
pub struct RelationMember {
    pub way_id: i64,
    pub role: MemberRole,
}

/// Multipolygon relation
#[derive(Clone)]
pub struct MultipolygonRelation {
    pub id: i64,
    pub members: Vec<RelationMember>,
    pub tags: Tags,
}

/// All extracted elements from PBF
pub struct ExtractedElements {
    pub nodes: NodeCache,
    pub coastline_ways: Vec<WayRef>,
    pub tidal_ways: Vec<WayRef>,
    pub water_ways: Vec<WayRef>,
    pub water_relations: Vec<MultipolygonRelation>,
    pub land_relations: Vec<MultipolygonRelation>,
}

/// Check if tags indicate a coastline
pub fn is_coastline(tags: &Tags) -> bool {
    tags.get("natural").map(|v| v == "coastline").unwrap_or(false)
}

/// Check if tags indicate a tidal feature
pub fn is_tidal(tags: &Tags) -> bool {
    tags.get("tidal").map(|v| v == "yes").unwrap_or(false)
}

/// Check if tags indicate a water feature
pub fn is_water_feature(tags: &Tags) -> bool {
    if let Some(natural) = tags.get("natural") {
        if natural == "water" || natural == "wetland" {
            return true;
        }
    }
    if let Some(waterway) = tags.get("waterway") {
        if waterway == "riverbank" || waterway == "dock" {
            return true;
        }
    }
    if let Some(landuse) = tags.get("landuse") {
        if landuse == "reservoir" || landuse == "basin" {
            return true;
        }
    }
    if tags.contains_key("water") {
        return true;
    }
    false
}

/// Check if a relation is a multipolygon
fn is_multipolygon(tags: &Tags) -> bool {
    tags.get("type").map(|v| v == "multipolygon").unwrap_or(false)
}

/// Extract all relevant elements from a PBF file
pub fn extract_elements(path: &Path, include_tidal: bool) -> Result<ExtractedElements> {
    log::info!("Reading PBF file: {}", path.display());

    let reader = ElementReader::from_path(path)
        .with_context(|| format!("Failed to open PBF file: {}", path.display()))?;

    // First pass: count elements for progress bar
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message("Counting elements...");

    let mut nodes = NodeCache::with_capacity(10_000_000);
    let mut coastline_ways = Vec::new();
    let mut tidal_ways = Vec::new();
    let mut water_ways = Vec::new();
    let mut water_relations = Vec::new();
    let mut land_relations = Vec::new();

    let mut node_count = 0u64;
    let mut way_count = 0u64;
    let mut relation_count = 0u64;

    reader.for_each(|element| {
        match element {
            Element::Node(node) => {
                nodes.insert(
                    node.id(),
                    Coord {
                        x: node.lon(),
                        y: node.lat(),
                    },
                );
                node_count += 1;
                if node_count % 1_000_000 == 0 {
                    pb.set_message(format!("Processed {} nodes...", node_count));
                }
            }
            Element::DenseNode(node) => {
                nodes.insert(
                    node.id(),
                    Coord {
                        x: node.lon(),
                        y: node.lat(),
                    },
                );
                node_count += 1;
                if node_count % 1_000_000 == 0 {
                    pb.set_message(format!("Processed {} nodes...", node_count));
                }
            }
            Element::Way(way) => {
                let tags: Tags = way
                    .tags()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();

                let way_ref = WayRef {
                    id: way.id(),
                    node_refs: way.refs().collect(),
                    tags: tags.clone(),
                };

                // Coastline ways
                if is_coastline(&tags) {
                    coastline_ways.push(way_ref.clone());
                }

                // Tidal features
                if include_tidal && is_tidal(&tags) {
                    tidal_ways.push(way_ref.clone());
                }

                // Water features (ways that form polygons directly)
                if is_water_feature(&tags) {
                    water_ways.push(way_ref);
                }

                way_count += 1;
                if way_count % 100_000 == 0 {
                    pb.set_message(format!(
                        "Processed {} nodes, {} ways...",
                        node_count, way_count
                    ));
                }
            }
            Element::Relation(rel) => {
                let tags: Tags = rel
                    .tags()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();

                if is_multipolygon(&tags) {
                    let members: Vec<RelationMember> = rel
                        .members()
                        .filter_map(|m| {
                            if m.member_type == osmpbf::RelMemberType::Way {
                                let role = match m.role() {
                                    Ok("outer") => MemberRole::Outer,
                                    Ok("inner") => MemberRole::Inner,
                                    _ => MemberRole::Unknown,
                                };
                                Some(RelationMember {
                                    way_id: m.member_id,
                                    role,
                                })
                            } else {
                                None
                            }
                        })
                        .collect();

                    if !members.is_empty() {
                        let mp_rel = MultipolygonRelation {
                            id: rel.id(),
                            members,
                            tags: tags.clone(),
                        };

                        if is_water_feature(&tags) {
                            water_relations.push(mp_rel);
                        } else if is_coastline(&tags) || is_tidal(&tags) {
                            land_relations.push(mp_rel);
                        }
                    }
                }

                relation_count += 1;
            }
        }
    })?;

    pb.finish_with_message(format!(
        "Processed {} nodes, {} ways, {} relations",
        node_count, way_count, relation_count
    ));

    log::info!("Node cache: {} nodes", nodes.len());
    log::info!("Coastline ways: {}", coastline_ways.len());
    log::info!("Tidal ways: {}", tidal_ways.len());
    log::info!("Water ways: {}", water_ways.len());
    log::info!("Water relations: {}", water_relations.len());
    log::info!("Land relations: {}", land_relations.len());

    Ok(ExtractedElements {
        nodes,
        coastline_ways,
        tidal_ways,
        water_ways,
        water_relations,
        land_relations,
    })
}

/// Resolve way node references to coordinates
pub fn resolve_way(way_ref: &WayRef, nodes: &NodeCache) -> Option<Way> {
    let coords: Option<Vec<Coord<f64>>> = way_ref
        .node_refs
        .iter()
        .map(|&id| nodes.get(id).copied())
        .collect();

    coords.map(|c| Way {
        id: way_ref.id,
        coords: c,
        tags: way_ref.tags.clone(),
    })
}

/// Resolve multiple ways
pub fn resolve_ways(way_refs: &[WayRef], nodes: &NodeCache) -> Vec<Way> {
    let mut resolved = Vec::with_capacity(way_refs.len());
    let mut missing = 0;

    for way_ref in way_refs {
        if let Some(way) = resolve_way(way_ref, nodes) {
            resolved.push(way);
        } else {
            missing += 1;
        }
    }

    if missing > 0 {
        log::warn!("{} ways could not be resolved (missing nodes)", missing);
    }

    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_water_feature() {
        let mut tags = Tags::new();
        tags.insert("natural".to_string(), "water".to_string());
        assert!(is_water_feature(&tags));

        tags.clear();
        tags.insert("waterway".to_string(), "riverbank".to_string());
        assert!(is_water_feature(&tags));

        tags.clear();
        tags.insert("landuse".to_string(), "reservoir".to_string());
        assert!(is_water_feature(&tags));

        tags.clear();
        tags.insert("water".to_string(), "lake".to_string());
        assert!(is_water_feature(&tags));

        tags.clear();
        tags.insert("natural".to_string(), "tree".to_string());
        assert!(!is_water_feature(&tags));
    }

    #[test]
    fn test_is_coastline() {
        let mut tags = Tags::new();
        tags.insert("natural".to_string(), "coastline".to_string());
        assert!(is_coastline(&tags));

        tags.clear();
        tags.insert("natural".to_string(), "water".to_string());
        assert!(!is_coastline(&tags));
    }

    #[test]
    fn test_way_ref_is_closed() {
        let way = WayRef {
            id: 1,
            node_refs: vec![1, 2, 3, 1],
            tags: Tags::new(),
        };
        assert!(way.is_closed());

        let way_open = WayRef {
            id: 2,
            node_refs: vec![1, 2, 3],
            tags: Tags::new(),
        };
        assert!(!way_open.is_closed());
    }
}
