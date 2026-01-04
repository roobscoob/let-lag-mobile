use std::collections::HashMap;

use roxmltree::Document;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = std::fs::read("input.kml")?;
    let xml = String::from_utf8_lossy(&data);

    let doc = Document::parse(&xml)?;

    let folder = doc
        .root()
        .first_child()
        .unwrap()
        .first_element_child()
        .unwrap()
        .first_element_child()
        .unwrap();

    for placemark in folder.children().filter(|n| n.has_tag_name("Placemark")) {
        let description = placemark
            .children()
            .find(|n| n.has_tag_name("description"))
            .unwrap()
            .text()
            .unwrap()
            .split("<BR>")
            .skip(2)
            .map(|s| {
                let mut split = s.split(" = ");
                let key = split.next().unwrap();
                let value = split.next().unwrap();

                if split.next().is_some() {
                    panic!("Unexpected extra '=' in description line");
                }

                (
                    key.trim_start_matches("<B>")
                        .trim_end_matches("</B>")
                        .to_lowercase(),
                    value,
                )
            })
            .filter(|(k, v)| !k.is_empty() && !v.is_empty() && k != "authors")
            .collect::<HashMap<_, _>>();

        let name = placemark
            .children()
            .find(|n| n.has_tag_name("name"))
            .unwrap()
            .text()
            .unwrap();

        let point = placemark
            .children()
            .find(|n| n.has_tag_name("Point"))
            .unwrap();

        let coordinates = point
            .children()
            .find(|n| n.has_tag_name("coordinates"))
            .unwrap()
            .text();

        if description.get("name") != Some(&name) {
            panic!("Name in description does not match name element");
        }

        if description.get("id").is_none() {
            println!("Name: {}", name);
            println!("Description: {:?}", description);
            println!("Coordinates: {}", coordinates.unwrap());
            println!("Missing 'id' in description");
        }

        // println!();
    }

    Ok(())
}
