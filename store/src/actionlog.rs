use nothing_action::act::Action;
use nothing_action::log::{ActionLog, AuthorId};

use crate::codec::{
    decode_op, decode_side, decode_ty, encode_op, encode_side, encode_ty, read_bool, read_i64,
    read_id, read_string, read_u8, read_u64, read_varint, write_bool, write_i64, write_id,
    write_string, write_u64, write_varint,
};
use crate::error::DecodeError;

fn encode_action_body(buf: &mut Vec<u8>, action: &Action) {
    match action {
        Action::MoveChild(n) => {
            buf.push(0);
            write_varint(buf, *n as u64);
        }
        Action::MoveParent => buf.push(1),
        Action::MoveNextSibling => buf.push(2),
        Action::MovePrevSibling => buf.push(3),
        Action::Delete => buf.push(4),
        Action::ConstructNum(n) => {
            buf.push(5);
            write_i64(buf, *n);
        }
        Action::ConstructBool(b) => {
            buf.push(6);
            write_bool(buf, *b);
        }
        Action::ConstructVar(id) => {
            buf.push(7);
            write_id(buf, *id);
        }
        Action::ConstructLam => buf.push(8),
        Action::ConstructAp => buf.push(9),
        Action::ConstructBinOp(op) => {
            buf.push(10);
            buf.push(encode_op(*op));
        }
        Action::ConstructIf => buf.push(11),
        Action::ConstructLet => buf.push(12),
        Action::ConstructPair => buf.push(13),
        Action::ConstructProj(side) => {
            buf.push(14);
            buf.push(encode_side(*side));
        }
        Action::ConstructNonEmptyHole => buf.push(15),
        Action::SetAnn(ty) => {
            buf.push(16);
            encode_ty(buf, ty);
        }
        Action::SetBinderId(id) => {
            buf.push(17);
            write_id(buf, *id);
        }
        Action::Rename(id, name) => {
            buf.push(18);
            write_id(buf, *id);
            write_string(buf, name);
        }
        Action::Finish => buf.push(19),
        Action::CreateDefinition => buf.push(20),
        Action::DeleteDefinition => buf.push(21),
        Action::SetDefAnn(ty) => {
            buf.push(22);
            encode_ty(buf, ty);
        }
        Action::MoveNextDef => buf.push(23),
        Action::MovePrevDef => buf.push(24),
        Action::MoveToDef(id) => {
            buf.push(25);
            write_id(buf, *id);
        }
        Action::ConstructStr(text) => {
            buf.push(26);
            write_string(buf, text);
        }
        Action::ConstructNil => buf.push(27),
        Action::ConstructCons => buf.push(28),
        Action::ConstructFold => buf.push(29),
        Action::ConstructRecord => buf.push(30),
        Action::ConstructField(id) => {
            buf.push(31);
            write_id(buf, *id);
        }
        Action::AddField => buf.push(32),
        Action::RemoveField => buf.push(33),
        Action::MoveFieldPrev => buf.push(34),
        Action::MoveFieldNext => buf.push(35),
        Action::SetField(id) => {
            buf.push(36);
            write_id(buf, *id);
        }
        Action::SetFieldId(id) => {
            buf.push(37);
            write_id(buf, *id);
        }
        Action::ConstructInj => buf.push(38),
        Action::ConstructMatch => buf.push(39),
        Action::AddArm => buf.push(40),
        Action::RemoveArm => buf.push(41),
        Action::SetConstructor(id) => {
            buf.push(42);
            write_id(buf, *id);
        }
        Action::SetArmBinderId(id) => {
            buf.push(43);
            write_id(buf, *id);
        }
        Action::ConstructPrint => buf.push(44),
        Action::ConstructReadline => buf.push(45),
        Action::ConstructPure => buf.push(46),
        Action::ConstructBind => buf.push(47),
    }
}

fn decode_action_body(bytes: &[u8], pos: &mut usize) -> Result<Action, DecodeError> {
    match read_u8(bytes, pos)? {
        0 => {
            let n = read_varint(bytes, pos)? as usize;
            Ok(Action::MoveChild(n))
        }
        1 => Ok(Action::MoveParent),
        2 => Ok(Action::MoveNextSibling),
        3 => Ok(Action::MovePrevSibling),
        4 => Ok(Action::Delete),
        5 => {
            let n = read_i64(bytes, pos)?;
            Ok(Action::ConstructNum(n))
        }
        6 => {
            let b = read_bool(bytes, pos)?;
            Ok(Action::ConstructBool(b))
        }
        7 => {
            let id = read_id(bytes, pos)?;
            Ok(Action::ConstructVar(id))
        }
        8 => Ok(Action::ConstructLam),
        9 => Ok(Action::ConstructAp),
        10 => {
            let op = decode_op(read_u8(bytes, pos)?)?;
            Ok(Action::ConstructBinOp(op))
        }
        11 => Ok(Action::ConstructIf),
        12 => Ok(Action::ConstructLet),
        13 => Ok(Action::ConstructPair),
        14 => {
            let side = decode_side(read_u8(bytes, pos)?)?;
            Ok(Action::ConstructProj(side))
        }
        15 => Ok(Action::ConstructNonEmptyHole),
        16 => {
            let ty = decode_ty(bytes, pos)?;
            Ok(Action::SetAnn(ty))
        }
        17 => {
            let id = read_id(bytes, pos)?;
            Ok(Action::SetBinderId(id))
        }
        18 => {
            let id = read_id(bytes, pos)?;
            let name = read_string(bytes, pos)?;
            Ok(Action::Rename(id, name))
        }
        19 => Ok(Action::Finish),
        20 => Ok(Action::CreateDefinition),
        21 => Ok(Action::DeleteDefinition),
        22 => {
            let ty = decode_ty(bytes, pos)?;
            Ok(Action::SetDefAnn(ty))
        }
        23 => Ok(Action::MoveNextDef),
        24 => Ok(Action::MovePrevDef),
        25 => {
            let id = read_id(bytes, pos)?;
            Ok(Action::MoveToDef(id))
        }
        26 => {
            let text = read_string(bytes, pos)?;
            Ok(Action::ConstructStr(text))
        }
        27 => Ok(Action::ConstructNil),
        28 => Ok(Action::ConstructCons),
        29 => Ok(Action::ConstructFold),
        30 => Ok(Action::ConstructRecord),
        31 => {
            let id = read_id(bytes, pos)?;
            Ok(Action::ConstructField(id))
        }
        32 => Ok(Action::AddField),
        33 => Ok(Action::RemoveField),
        34 => Ok(Action::MoveFieldPrev),
        35 => Ok(Action::MoveFieldNext),
        36 => {
            let id = read_id(bytes, pos)?;
            Ok(Action::SetField(id))
        }
        37 => {
            let id = read_id(bytes, pos)?;
            Ok(Action::SetFieldId(id))
        }
        38 => Ok(Action::ConstructInj),
        39 => Ok(Action::ConstructMatch),
        40 => Ok(Action::AddArm),
        41 => Ok(Action::RemoveArm),
        42 => {
            let id = read_id(bytes, pos)?;
            Ok(Action::SetConstructor(id))
        }
        44 => Ok(Action::ConstructPrint),
        45 => Ok(Action::ConstructReadline),
        46 => Ok(Action::ConstructPure),
        47 => Ok(Action::ConstructBind),
        43 => {
            let id = read_id(bytes, pos)?;
            Ok(Action::SetArmBinderId(id))
        }
        other => Err(DecodeError::BadTag(other)),
    }
}

pub fn encode_log(buf: &mut Vec<u8>, log: &ActionLog) {
    write_varint(buf, log.entries().len() as u64);
    for entry in log.entries() {
        let mut body = Vec::new();
        write_u64(&mut body, entry.timestamp);
        write_u64(&mut body, entry.author.0);
        encode_action_body(&mut body, &entry.action);
        write_varint(buf, body.len() as u64);
        buf.extend_from_slice(&body);
    }
}

pub fn decode_log(bytes: &[u8], pos: &mut usize) -> Result<ActionLog, DecodeError> {
    let count = read_varint(bytes, pos)?;
    let mut log = ActionLog::new();
    for _ in 0..count {
        let entry_len = read_varint(bytes, pos)? as usize;
        let start = *pos;
        let timestamp = read_u64(bytes, pos)?;
        let author = read_u64(bytes, pos)?;
        let action = decode_action_body(bytes, pos)?;
        if *pos - start != entry_len {
            return Err(DecodeError::TrailingBytes);
        }
        log.append(action, timestamp, AuthorId::new(author));
    }
    Ok(log)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_core::exp::{Id, Op};

    #[test]
    fn a_log_round_trips() {
        let mut log = ActionLog::new();
        log.append(Action::ConstructNum(5), 1_000, AuthorId::new(1));
        log.append(
            Action::Rename(Id::from_u128(9), "items".to_string()),
            2_000,
            AuthorId::new(2),
        );
        log.append(Action::ConstructBinOp(Op::Add), 3_000, AuthorId::new(3));

        let mut buf = Vec::new();
        encode_log(&mut buf, &log);
        let mut pos = 0;
        let decoded = decode_log(&buf, &mut pos).unwrap();

        assert_eq!(decoded, log);
        assert_eq!(pos, buf.len());
    }

    #[test]
    fn every_field_action_round_trips_through_the_log() {
        let field = Id::from_u128(4);
        let mut log = ActionLog::new();
        for (i, action) in [
            Action::ConstructRecord,
            Action::ConstructField(field),
            Action::AddField,
            Action::RemoveField,
            Action::MoveFieldPrev,
            Action::MoveFieldNext,
            Action::SetField(field),
            Action::SetFieldId(field),
        ]
        .into_iter()
        .enumerate()
        {
            log.append(action, i as u64, AuthorId::new(1));
        }

        let mut buf = Vec::new();
        encode_log(&mut buf, &log);
        let mut pos = 0;
        assert_eq!(decode_log(&buf, &mut pos).unwrap(), log);
        assert_eq!(pos, buf.len());
    }

    #[test]
    fn every_variant_action_round_trips_through_the_log() {
        let ctor = Id::from_u128(5);
        let binder = Id::from_u128(6);
        let mut log = ActionLog::new();
        for (i, action) in [
            Action::ConstructInj,
            Action::ConstructMatch,
            Action::AddArm,
            Action::RemoveArm,
            Action::SetConstructor(ctor),
            Action::SetArmBinderId(binder),
        ]
        .into_iter()
        .enumerate()
        {
            log.append(action, i as u64, AuthorId::new(1));
        }

        let mut buf = Vec::new();
        encode_log(&mut buf, &log);
        let mut pos = 0;
        assert_eq!(decode_log(&buf, &mut pos).unwrap(), log);
        assert_eq!(pos, buf.len());
    }
}
