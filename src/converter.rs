use crate::{botw, model, util};
use byteordered::Endianness;
use failure::ResultExt;
use indexmap::IndexMap;
use msbt::builder::MsbtBuilder;
use msbt::section::Atr1;
use msbt::Msbt;
use model::Msyt;
use model::{Content, Entry, MsbtInfo};
use std::fs::OpenOptions;
use std::io::{BufReader,  Cursor, Read, Write};
use std::panic::AssertUnwindSafe;
use std::{fs, io, panic};


pub type MsbtResult<T> = std::result::Result<T, failure::Error>;

pub fn str_endian_to_byteordered(endian: &str) -> Endianness {
    match endian {
        "BE" => {return Endianness::Big;},
        "LE" => {return Endianness::Little;},
        _ => {return Endianness::Little;}
    }
}
pub struct MsytFile {}

impl MsytFile {
    pub fn text_to_binary_file(text: &str, path: &str, endian: String) -> io::Result<()> {
        let encoding = msbt::Encoding::Utf16;

        /*let result = panic::catch_unwind(AssertUnwindSafe(|| {
            MsytFile::text_to_binary(text, endian, Some(encoding)).expect("Unable to save to msyt")
        }));*/
        match MsytFile::text_to_binary_safe(text, endian, Some(encoding)) {
            Ok(data) => {
                let mut file = OpenOptions::new().write(true).open(&path)?;
                file.write_all(&data).expect("Error writing data to file");
                return Ok(());
            },
            //_ => {return Err(failure::format_err!("Not msyt file"));}
            Err(err) => {
                let e = format!("Unable to convert text to binary file:\n{}\n{}", &path, err);
                return Err(io::Error::new(io::ErrorKind::InvalidData, e));
            }
        }

        //let data = MsytFile::text_to_binary(text, endian, Some(encoding)).expect("Unable to save to msyt");
        //let mut f_handle = fs::File::open(&path).expect(&format!("Failed to open file {}", &path));
    }

    pub fn file_to_text(path: String) -> io::Result<String> {
        let mut f_handle = fs::File::open(&path)?;
        let mut buf: Vec<u8> = Vec::new();
        f_handle.read_to_end(&mut buf)?;
        match MsytFile::binary_to_text_safe(buf) {
            Ok(text) => {return Ok(text);},
            Err(err) => {
                let e = format!("Unable to convert data from file to text:\n{}\n{}", &path, err);
                return Err(io::Error::new(io::ErrorKind::InvalidData, e));
            }
        }
        //let text = MsytFile::binary_to_text(buf)?;
        //Ok(text)
    }

    pub fn text_to_binary_safe(text: &str, endian: String, encoding_type: Option<msbt::Encoding>) -> io::Result<Vec<u8>> {
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            MsytFile::text_to_binary(text, endian, encoding_type)
        }));
        if let Ok(bytes_result) = &result {
            if let Ok(raw_data) = &bytes_result {
                return Ok(raw_data.to_vec());
            }
        }
        return Err(io::Error::new(io::ErrorKind::InvalidData, "unable to convert text to binary msbt"));

    }

    pub fn binary_to_text_safe(data: Vec<u8>) -> io::Result<String> {
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            MsytFile::binary_to_text(data)
        }));
        if let Ok(text_result) = &result {
            if let Ok(text) = &text_result {
                return Ok(text.to_string());
            }
        }
        return Err(io::Error::new(io::ErrorKind::InvalidData, "unable to convert data to msbt"));

    }

    pub fn binary_to_text(data: Vec<u8>) -> MsbtResult<String> {
        if !data.starts_with(b"MsgStd") {
            return Err(failure::format_err!("Not msyt file"));
        }
        let cursor = Cursor::new(&data);
        let reader: BufReader<Cursor<&Vec<u8>>> = BufReader::new(cursor);
        let msbt = Msbt::from_reader(BufReader::new(reader))
            .with_context(|_| format!("Failed to create msbt from reader"))
            .expect(&format!("Failed to create msbt from reader"));

        let lbl1 = match msbt.lbl1() {
            Some(lbl) => lbl,
            None => {
                println!("invalid msbt: missing lbl1: ");
                return Ok("".to_string());
            }
        };

        let mut entries = IndexMap::with_capacity(lbl1.labels().len());

        for label in lbl1.labels() {
            let mut all_content = Vec::new();

            let raw_value = label
                .value_raw()
                .ok_or_else(|| {
                    failure::format_err!(
                        "invalid msbt at : missing string for label {}",
                        label.name(),
                    )
                })
                .expect(&format!(
                    "invalid msbt at : missing string for label {}",
                    label.name(),
                ));
            if let Ok(parts) = &mut botw::parse_controls(msbt.header(), raw_value) {
                all_content.append(parts);

            }
            /*let mut parts = botw::parse_controls(msbt.header(), raw_value)
                .expect("Failed to parse controls");
            all_content.append(&mut parts);*/
            let entry = Entry {
                attributes: msbt.atr1().and_then(|a| {
                    a.strings()
                        .get(label.index() as usize)
                        .map(|s| util::strip_nul(*s))
                        .map(ToString::to_string)
                }),
                contents: all_content,
            };
            entries.insert(label.name().to_string(), entry);
        }

        entries.sort_keys();

        let msyt = Msyt {
            entries,
            msbt: MsbtInfo {
                group_count: lbl1.group_count(),
                atr1_unknown: msbt.atr1().map(Atr1::unknown_1),
                ato1: msbt.ato1().map(|a| a.unknown_bytes().to_vec()),
                tsy1: msbt.tsy1().map(|a| a.unknown_bytes().to_vec()),
                nli1: msbt.nli1().map(|a| model::Nli1 {
                    id_count: a.id_count(),
                    global_ids: a.global_ids().clone(),
                }),
            },
        };

        let yaml_string =
            serde_yaml::to_string(&msyt).with_context(|_| "could not serialize yaml to string")?;
        Ok(yaml_string)
    }

    //pub fn text_to_binary(text: &str, endianness: Endianness, encoding: msbt::Encoding) -> MsbtResult<Vec<u8>> {
    pub fn text_to_binary(text: &str, endian: String, encoding_type: Option<msbt::Encoding>) -> MsbtResult<Vec<u8>> {
        let encoding = encoding_type.unwrap_or(msbt::Encoding::Utf16);
        let endianness = str_endian_to_byteordered(&endian);
        let msyt: Msyt =
            serde_yaml::from_str(&text).expect(&format!("Cannot create msyt from string"));

        let mut builder = MsbtBuilder::new(endianness, encoding, Some(msyt.msbt.group_count));
      if let Some(unknown_bytes) = msyt.msbt.ato1 {
        builder = builder.ato1(msbt::section::Ato1::new_unlinked(unknown_bytes));
      }
      if let Some(unknown_1) = msyt.msbt.atr1_unknown {
        // ATR1 should have exactly the same amount of entries as TXT2. In the BotW files, sometimes
        // an ATR1 section is specified to have that amount but the section is actually empty. For
        // msyt's purposes, if the msyt does not contain the same amount of attributes as it does
        // text entries (i.e. not every label has an `attributes` node), it will be assumed that the
        // ATR1 section should specify that it has the correct amount of entries but actually be
        // empty.
        let strings: Option<Vec<String>> = msyt.entries
          .iter()
          .map(|(_, e)| e.attributes.clone())
          .map(|s| s.map(util::append_nul))
          .collect();
        let atr_len = match strings {
          Some(ref s) => s.len(),
          None => msyt.entries.len(),
        };
        let strings = strings.unwrap_or_default();
        builder = builder.atr1(msbt::section::Atr1::new_unlinked(atr_len as u32, unknown_1, strings));
      }
      if let Some(unknown_bytes) = msyt.msbt.tsy1 {
        builder = builder.tsy1(msbt::section::Tsy1::new_unlinked(unknown_bytes));
      }
      if let Some(nli1) = msyt.msbt.nli1 {
        builder = builder.nli1(msbt::section::Nli1::new_unlinked(nli1.id_count, nli1.global_ids));
      }
      for (label, entry) in msyt.entries.into_iter() {
        let new_val = Content::write_all(builder.header(), &entry.contents)?;
        builder = builder.add_label(label, new_val);
      }
      let msbt = builder.build();

      let mut dest_buf: Vec<u8> = Vec::new();
      let dest_cursor = Cursor::new(&mut dest_buf);
      //let mut new_msbt = Msbt::from_reader(dest_cursor).expect("Failed to create empty msbt");
      msbt.write_to(dest_cursor).expect(&format!("Error writing to cursor {}", line!()));

        Ok(dest_buf)
    }
}
