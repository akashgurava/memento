use std::fs::File;
use std::io::BufReader;

use super::MetadataEntry;

/// Extract EXIF metadata from an image file using kamadak-exif
pub fn extract_image_metadata(path: &str) -> Vec<MetadataEntry> {
    let mut entries = Vec::new();

    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return entries,
    };

    let reader = match exif::Reader::new().read_from_container(&mut BufReader::new(file)) {
        Ok(r) => r,
        Err(_) => return entries,
    };

    for field in reader.fields() {
        let tag_name = format!("{}", field.tag);
        let namespace = match field.ifd_num {
            exif::In::PRIMARY => "exif".to_string(),
            exif::In::THUMBNAIL => "exif_thumb".to_string(),
            _ => "exif_other".to_string(),
        };

        let value = extract_field_value(field);
        entries.push((namespace, tag_name, value));
    }

    entries
}

fn extract_field_value(field: &exif::Field) -> Option<String> {
    match &field.value {
        exif::Value::Byte(v) => {
            if v.len() == 1 {
                Some(v[0].to_string())
            } else {
                Some(format!("{:?}", v))
            }
        }
        exif::Value::Short(v) => {
            if v.len() == 1 {
                Some(v[0].to_string())
            } else {
                Some(format!("{:?}", v))
            }
        }
        exif::Value::Long(v) => {
            if v.len() == 1 {
                Some(v[0].to_string())
            } else {
                Some(format!("{:?}", v))
            }
        }
        exif::Value::Rational(v) => {
            if v.len() == 1 {
                let r = v[0];
                if r.denom != 0 {
                    Some(format!("{}", r.num as f64 / r.denom as f64))
                } else {
                    Some(format!("{}/{}", r.num, r.denom))
                }
            } else {
                Some(field.display_value().to_string())
            }
        }
        exif::Value::SRational(v) => {
            if v.len() == 1 {
                let r = v[0];
                if r.denom != 0 {
                    Some(format!("{}", r.num as f64 / r.denom as f64))
                } else {
                    Some(format!("{}/{}", r.num, r.denom))
                }
            } else {
                Some(field.display_value().to_string())
            }
        }
        exif::Value::Ascii(v) => {
            let text = v
                .iter()
                .filter_map(|s| std::str::from_utf8(s).ok())
                .collect::<Vec<_>>()
                .join(", ");
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        }
        exif::Value::Undefined(v, _) => {
            if v.len() <= 64 {
                Some(format!("{:?}", v))
            } else {
                Some(format!("[{} bytes]", v.len()))
            }
        }
        _ => Some(field.display_value().to_string()),
    }
}
