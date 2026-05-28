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

        // Try to extract value in the most appropriate form
        let (value_text, value_int, value_real) = extract_field_value(field);

        entries.push((namespace, tag_name, value_text, value_int, value_real));
    }

    entries
}

fn extract_field_value(field: &exif::Field) -> (Option<String>, Option<i64>, Option<f64>) {
    match &field.value {
        exif::Value::Byte(v) => {
            if v.len() == 1 {
                (None, Some(v[0] as i64), None)
            } else {
                (Some(format!("{:?}", v)), None, None)
            }
        }
        exif::Value::Short(v) => {
            if v.len() == 1 {
                (None, Some(v[0] as i64), None)
            } else {
                (Some(format!("{:?}", v)), None, None)
            }
        }
        exif::Value::Long(v) => {
            if v.len() == 1 {
                (None, Some(v[0] as i64), None)
            } else {
                (Some(format!("{:?}", v)), None, None)
            }
        }
        exif::Value::Rational(v) => {
            if v.len() == 1 {
                let r = v[0];
                if r.denom != 0 {
                    (None, None, Some(r.num as f64 / r.denom as f64))
                } else {
                    (Some(format!("{}/{}", r.num, r.denom)), None, None)
                }
            } else {
                (Some(field.display_value().to_string()), None, None)
            }
        }
        exif::Value::SRational(v) => {
            if v.len() == 1 {
                let r = v[0];
                if r.denom != 0 {
                    (None, None, Some(r.num as f64 / r.denom as f64))
                } else {
                    (Some(format!("{}/{}", r.num, r.denom)), None, None)
                }
            } else {
                (Some(field.display_value().to_string()), None, None)
            }
        }
        exif::Value::Ascii(v) => {
            let text = v
                .iter()
                .filter_map(|s| std::str::from_utf8(s).ok())
                .collect::<Vec<_>>()
                .join(", ");
            (Some(text), None, None)
        }
        exif::Value::Undefined(v, _) => {
            if v.len() <= 64 {
                (Some(format!("{:?}", v)), None, None)
            } else {
                (Some(format!("[{} bytes]", v.len())), None, None)
            }
        }
        _ => (Some(field.display_value().to_string()), None, None),
    }
}
