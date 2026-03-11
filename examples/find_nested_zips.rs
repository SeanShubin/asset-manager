use std::io::Read;

fn main() {
    let roots = &[
        "D:/keep/assets/original/finalbossblues.itch.io",
        "D:/keep/assets/original/lynocs.itch.io",
        "D:/keep/assets/original/smallscaleint.itch.io",
    ];

    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            eprintln!("Cannot read {root}");
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("zip") {
                continue;
            }
            let Ok(file) = std::fs::File::open(&path) else {
                continue;
            };
            let Ok(mut archive) = zip::ZipArchive::new(file) else {
                continue;
            };
            let mut found = false;
            for i in 0..archive.len() {
                let Ok(entry) = archive.by_index(i) else {
                    continue;
                };
                let name = entry.name().to_string();
                if name.to_lowercase().ends_with(".zip") {
                    if !found {
                        println!("\n{}", path.display());
                        found = true;
                    }
                    println!("  nested: {name} ({} bytes)", entry.size());

                    // Test reading the inner zip
                    drop(entry);
                    let Ok(mut ze) = archive.by_name(&name) else {
                        continue;
                    };
                    let mut buf = Vec::new();
                    if ze.read_to_end(&mut buf).is_ok() {
                        drop(ze);
                        let cursor = std::io::Cursor::new(buf);
                        match zip::ZipArchive::new(cursor) {
                            Ok(inner) => {
                                let images: usize = (0..inner.len())
                                    .filter(|&j| {
                                        inner.name_for_index(j).is_some_and(|n| {
                                            let l = n.to_ascii_lowercase();
                                            l.ends_with(".png") || l.ends_with(".jpg") || l.ends_with(".bmp")
                                        })
                                    })
                                    .count();
                                println!("    -> {} entries, {} images", inner.len(), images);
                            }
                            Err(e) => println!("    -> invalid inner zip: {e}"),
                        }
                    }
                }
            }
        }
    }
}
