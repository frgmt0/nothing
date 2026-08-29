use uuid::Uuid;

use nothing_core::exp::{HoleId, Id, Op, Side};
use nothing_core::ty::Ty;

use crate::error::DecodeError;

pub fn write_varint(buf: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            buf.push(byte);
            break;
        }
        buf.push(byte | 0x80);
    }
}

pub fn read_varint(bytes: &[u8], pos: &mut usize) -> Result<u64, DecodeError> {
    let mut result: u64 = 0;
    let mut shift: u32 = 0;
    loop {
        let byte = read_u8(bytes, pos)?;
        if shift >= 64 {
            return Err(DecodeError::VarintTooLong);
        }
        result |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    Ok(result)
}

pub fn read_u8(bytes: &[u8], pos: &mut usize) -> Result<u8, DecodeError> {
    let byte = *bytes.get(*pos).ok_or(DecodeError::UnexpectedEof)?;
    *pos += 1;
    Ok(byte)
}

pub fn read_bytes<'a>(
    bytes: &'a [u8],
    pos: &mut usize,
    len: usize,
) -> Result<&'a [u8], DecodeError> {
    let end = pos.checked_add(len).ok_or(DecodeError::UnexpectedEof)?;
    let slice = bytes.get(*pos..end).ok_or(DecodeError::UnexpectedEof)?;
    *pos = end;
    Ok(slice)
}

pub fn write_i64(buf: &mut Vec<u8>, n: i64) {
    buf.extend_from_slice(&n.to_le_bytes());
}

pub fn read_i64(bytes: &[u8], pos: &mut usize) -> Result<i64, DecodeError> {
    let slice = read_bytes(bytes, pos, 8)?;
    let arr: [u8; 8] = slice.try_into().expect("read_bytes returned 8 bytes");
    Ok(i64::from_le_bytes(arr))
}

pub fn write_u64(buf: &mut Vec<u8>, n: u64) {
    buf.extend_from_slice(&n.to_le_bytes());
}

pub fn read_u64(bytes: &[u8], pos: &mut usize) -> Result<u64, DecodeError> {
    let slice = read_bytes(bytes, pos, 8)?;
    let arr: [u8; 8] = slice.try_into().expect("read_bytes returned 8 bytes");
    Ok(u64::from_le_bytes(arr))
}

pub fn write_bool(buf: &mut Vec<u8>, b: bool) {
    buf.push(if b { 1 } else { 0 });
}

pub fn read_bool(bytes: &[u8], pos: &mut usize) -> Result<bool, DecodeError> {
    match read_u8(bytes, pos)? {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(DecodeError::BadBool(other)),
    }
}

pub fn write_uuid_bytes(buf: &mut Vec<u8>, uuid: Uuid) {
    buf.extend_from_slice(uuid.as_bytes());
}

pub fn read_uuid(bytes: &[u8], pos: &mut usize) -> Result<Uuid, DecodeError> {
    let slice = read_bytes(bytes, pos, 16)?;
    let arr: [u8; 16] = slice.try_into().expect("read_bytes returned 16 bytes");
    Ok(Uuid::from_bytes(arr))
}

pub fn write_id(buf: &mut Vec<u8>, id: Id) {
    write_uuid_bytes(buf, id.uuid());
}

pub fn read_id(bytes: &[u8], pos: &mut usize) -> Result<Id, DecodeError> {
    read_uuid(bytes, pos).map(Id::from_uuid)
}

pub fn write_hole_id(buf: &mut Vec<u8>, id: HoleId) {
    write_uuid_bytes(buf, id.uuid());
}

pub fn read_hole_id(bytes: &[u8], pos: &mut usize) -> Result<HoleId, DecodeError> {
    read_uuid(bytes, pos).map(HoleId::from_uuid)
}

pub fn write_string(buf: &mut Vec<u8>, s: &str) {
    write_varint(buf, s.len() as u64);
    buf.extend_from_slice(s.as_bytes());
}

pub fn read_string(bytes: &[u8], pos: &mut usize) -> Result<String, DecodeError> {
    let len = read_varint(bytes, pos)? as usize;
    let slice = read_bytes(bytes, pos, len)?;
    std::str::from_utf8(slice)
        .map(str::to_string)
        .map_err(|_| DecodeError::BadUtf8)
}

pub fn encode_op(op: Op) -> u8 {
    match op {
        Op::Add => 0,
        Op::Sub => 1,
        Op::Mul => 2,
        Op::Lt => 3,
        Op::Eq => 4,
        Op::Concat => 5,
    }
}

pub fn decode_op(tag: u8) -> Result<Op, DecodeError> {
    match tag {
        0 => Ok(Op::Add),
        1 => Ok(Op::Sub),
        2 => Ok(Op::Mul),
        3 => Ok(Op::Lt),
        4 => Ok(Op::Eq),
        5 => Ok(Op::Concat),
        other => Err(DecodeError::BadTag(other)),
    }
}

pub fn encode_side(side: Side) -> u8 {
    match side {
        Side::L => 0,
        Side::R => 1,
    }
}

pub fn decode_side(tag: u8) -> Result<Side, DecodeError> {
    match tag {
        0 => Ok(Side::L),
        1 => Ok(Side::R),
        other => Err(DecodeError::BadTag(other)),
    }
}

pub fn encode_ty(buf: &mut Vec<u8>, ty: &Ty) {
    match ty {
        Ty::Num => buf.push(0),
        Ty::Bool => buf.push(1),
        Ty::Arrow(a, b) => {
            buf.push(2);
            encode_ty(buf, a);
            encode_ty(buf, b);
        }
        Ty::Prod(a, b) => {
            buf.push(3);
            encode_ty(buf, a);
            encode_ty(buf, b);
        }
        Ty::Hole => buf.push(4),
        Ty::Str => buf.push(5),
        Ty::List(elem) => {
            buf.push(6);
            encode_ty(buf, elem);
        }
    }
}

pub fn decode_ty(bytes: &[u8], pos: &mut usize) -> Result<Ty, DecodeError> {
    match read_u8(bytes, pos)? {
        0 => Ok(Ty::Num),
        1 => Ok(Ty::Bool),
        2 => {
            let a = decode_ty(bytes, pos)?;
            let b = decode_ty(bytes, pos)?;
            Ok(Ty::Arrow(Box::new(a), Box::new(b)))
        }
        3 => {
            let a = decode_ty(bytes, pos)?;
            let b = decode_ty(bytes, pos)?;
            Ok(Ty::Prod(Box::new(a), Box::new(b)))
        }
        4 => Ok(Ty::Hole),
        5 => Ok(Ty::Str),
        6 => Ok(Ty::List(Box::new(decode_ty(bytes, pos)?))),
        other => Err(DecodeError::BadTag(other)),
    }
}
