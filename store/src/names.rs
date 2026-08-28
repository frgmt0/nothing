use nothing_core::names::NameTable;

use crate::codec::{read_id, read_string, read_varint, write_id, write_string, write_varint};
use crate::error::DecodeError;

pub fn encode_names(buf: &mut Vec<u8>, names: &NameTable) {
    let mut entries = names.flatten().entries();
    entries.sort_by_key(|(id, _)| id.as_u128());
    write_varint(buf, entries.len() as u64);
    for (id, name) in &entries {
        write_id(buf, *id);
        write_string(buf, name);
    }
}

pub fn decode_names(bytes: &[u8], pos: &mut usize) -> Result<NameTable, DecodeError> {
    let count = read_varint(bytes, pos)?;
    let mut names = NameTable::new();
    for _ in 0..count {
        let id = read_id(bytes, pos)?;
        let name = read_string(bytes, pos)?;
        names.set(id, name);
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_core::exp::Id;

    #[test]
    fn a_flattened_overlay_round_trips_as_one_layer() {
        let mut base = NameTable::new();
        base.set(Id::from_u128(1), "xs");
        let mut overlay = NameTable::overlay(&base);
        overlay.set(Id::from_u128(1), "items");
        overlay.set(Id::from_u128(2), "n");

        let mut buf = Vec::new();
        encode_names(&mut buf, &overlay);
        let mut pos = 0;
        let decoded = decode_names(&buf, &mut pos).unwrap();

        assert_eq!(decoded.depth(), 1);
        assert_eq!(decoded.get(Id::from_u128(1)), Some("items"));
        assert_eq!(decoded.get(Id::from_u128(2)), Some("n"));
    }
}
