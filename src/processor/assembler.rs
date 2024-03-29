use pyo3::prelude::*;
use std::{fs, str::FromStr};

#[repr(u16)]
#[derive(EnumString, FromRepr, Debug)]
pub enum OpCodes {
    IRQ, //0
    LDA, //1
    STA, //2
    ADD, //3
    SUB, //4
    MUL, //5
    DIV, //6
    CMP, //7
    NEG, //8
    BEQ, //9
    BGT, //10
    BLT, //11
    BHS, //12
    BMI, //13
    BVS, //14
    BHI, //15
    PSH, //16
    POP, //17
    JAL, //18
    JMP, //19
    AND, //20
    ORR, //21
    NOT, //22
    XOR, //23
    LSL, //24
    LSR, //25
    ASL, //26
    ASR, //27
    ROR, //28
    RCR, //29
    CLZ, //30
    RET, //31
    REM, //32
}

#[repr(u8)]
#[derive(EnumString, PartialEq, FromRepr)]
pub enum PseudoOps {
    HALT,
    PRINT,
    READ,
    SET,
    CLEAR,
    BEGIN,
    END,
    EXTERN,
}

#[pyfunction]
pub fn assemble(in_asm: &str, breadcrumb: Option<&str>) -> PyResult<(bool, String)> {
    let s = match fs::read_to_string(in_asm) {
        Ok(s) => s,
        Err(why) => return Ok((false, why.to_string())),
    };

    let mut buf = String::with_capacity(s.len());
    buf.push('\n');

    let mut began = false;
    let mut ended = false;

    let mut header_len: u32 = 1;
    let mut offset = 0;

    match s.lines().enumerate().try_for_each(|(i, line)| {
        let line = match line.split_once("//") {
            Some((code, _comment)) => code.trim(),
            None => line.trim()
        };

        if !line.is_empty() {
            if ended {
                return Err("File continues after END".to_owned());
            } else if let Some((label, text)) = line.split_once(".text") {
                if began {
                    return Err(format!("Found .text directive after BEGIN statement at line {}", i + 1));
                }

                let label = label.trim_end();
                match label.strip_suffix(':') {
                    None => return Err(format!("Expected label before .text directive at line {}", i + 1)),

                    Some(label) => match label.trim_end().chars().any(|c| c.is_whitespace()) {
                        true => return Err(format!("Found whitespace in label at line {}\n\t{}", i + 1, label)),

                        false => if buf.lines().take(match usize::try_from(header_len) {
                            Err(_) => return Err("16 bit architecture unsupported".to_owned()),

                            Ok(v) => v,
                        }).any(|line| line.starts_with(label)) {
                            return Err(format!("Found label redefinition at line {}\n\t{}", i + 1, label));
                        }
                    }
                }

                let text = match text.strip_prefix(|c: char| c.is_whitespace()) {
                    None => return Err(format!("Expected whitespace after .text directive at line {}", i + 1)),

                    Some(text) => text,
                };

                buf.extend(label.chars().filter(|c| !c.is_whitespace()));
                buf.push_str(text);
                buf.push_str("\"\n");
                header_len += 1;
            } else if let Some((label, words)) = line.split_once(".word") {
                if began {
                    return Err(format!("Found .word directive after BEGIN statement at line {}", i + 1));
                }

                let label = label.trim_end();
                match label.strip_suffix(':') {
                    None => return Err(format!("Expected label before .word directive at line {}", i + 1)),

                    Some(label) => match label.trim_end().chars().any(|c| c.is_whitespace()) {
                        true => return Err(format!("Found whitespace in label at line {}\n\t{}", i + 1, label)),

                        false => if buf.lines().take(match usize::try_from(header_len) {
                            Err(_) => return Err("16 bit architecture unsupported".to_owned()),

                            Ok(v) => v,
                        }).any(|line| line.starts_with(label)) {
                            return Err(format!("Found label redefinition at line {}\n\t{}", i + 1, label));
                        }
                    }
                }

                match words.starts_with(|c: char| c.is_whitespace()) {
                    false => return Err(format!("Expected whitespace after .word directive at line {}", i + 1)),

                    true => words.split(',').try_for_each(|word| match word.trim().parse::<u32>() {
                        Err(_) => Err(format!("Couldn't parse word at line {}\n\t{}", i + 1, word)),
                        Ok(_) => Ok(()),
                    })?,
                }

                buf.extend(label.chars().chain(words.chars()).filter(|c| !c.is_whitespace()));
                buf.push('\n');
                header_len += 1;
            } else if let Some((label, text)) = line.split_once(':') {
                if !began {
                    return Err(format!("Expected directive after label at line {}", i + 1));
                }

                let label = label.trim_end();
                match label.chars().any(|c| c.is_whitespace()) {
                    true => return Err(format!("Found whitespace in label at line {}\n\t{}", i + 1, label)),

                    false => if buf.lines().take(match usize::try_from(header_len) {
                        Err(_) => return Err("16 bit architecture unsupported".to_owned()),

                        Ok(v) => v,
                    }).any(|line| line.starts_with(label)) {
                        return Err(format!("Label {} found at line {} previously defined", label, i + 1));
                    }
                }

                let string = label.to_owned() + " " + match u32::try_from(i - match usize::try_from(offset) {
                    Err(_) => return Err("16 bit architecture unsupported".to_owned()),

                    Ok(v) => v,
                }) {
                    Err(_) => return Err("File too big!".to_owned()),

                    Ok(v) => {
                        if v.leading_zeros() < 7 {
                            return Err("File too big!".to_owned());
                        }

                        v
                    }
                }.to_string().as_str() + "\n";

                buf.insert_str(buf.lines().take(match usize::try_from(header_len) {
                    Err(_) => return Err("16 bit architecture unsupported".to_owned()),

                    Ok(v) => v,
                }).map(|line| line.bytes().count() + 1).reduce(|acc, n| acc + n).unwrap_or(0), string.as_str());
                header_len += 1;

                let mut tokens = text.split_whitespace();
                match tokens.next() {
                    None => offset += 1,
                    Some(token) => {
                        if let Ok(op) = OpCodes::from_str(token) {
                            match op {
                                OpCodes::IRQ => match tokens.next() {
                                    None => return Err(format!("Expected integer at line {}", i + 1)),

                                    Some(irq_type) => match irq_type.parse::<u8>() {
                                        Err(_) => return Err(format!("Expected integer at line {}\n\tfound {} instead", i + 1, irq_type)),

                                        Ok(v) => match tokens.next() {
                                            Some(arg) => match v {
                                                1..=3 => {
                                                    buf.push_str("IRQ ");
                                                    buf.push_str(irq_type);
                                                    buf.push(' ');
                                                    buf.push_str(arg);
                                                    buf.push('\n');
                                                }
                                                0 | 4 => return Err(format!("Unexpected argument at line {}\n\t{}", i + 1, token)),
                                                _ => return Err(format!("Unknown IRQ type at line {}\n\t{}", i + 1, irq_type)),
                                            }
                                            None => match v {
                                                0 | 4 => {
                                                    buf.push_str("IRQ ");
                                                    buf.push_str(irq_type);
                                                    buf.push('\n');
                                                }
                                                1..=3 => return Err(format!("Expected label at line {}", i + 1)),
                                                _ => return Err(format!("Unknown IRQ type at line {}\n\t{}", i + 1, irq_type)),
                                            }
                                        }
                                    }
                                }
                                _ => match tokens.next() {
                                    None => return Err(format!("Expected label at line {}", i + 1)),

                                    Some(arg) => {
                                        buf.push_str(token);
                                        buf.push(' ');
                                        buf.push_str(arg);
                                        buf.push('\n');
                                    }
                                }
                            }
                        } else {
                            match PseudoOps::from_str(token) {
                                Err(_) => return Err(format!("Expected instruction at line {}\t\nfound {} instead", i + 1, token)),

                                Ok(psop) => match psop {
                                    PseudoOps::EXTERN => return Err(format!("Found EXTERN statement after BEGIN at line {}", i + 1)),

                                    PseudoOps::BEGIN => return Err(format!("Found repeated BEGIN statement at line {}", i + 1)),

                                    PseudoOps::END => ended = true,

                                    PseudoOps::SET => match tokens.next() {
                                        None => return Err(format!("Expected label at line {}", i + 1)),

                                        Some(arg) => match u32::from_str_radix(arg, 2) {
                                            Err(_) => return Err(format!("Expected binary number as argument at line {}\n\tfound {} instead", i + 1, token)),

                                            Ok(_) => {
                                                buf.push_str("IRQ 3 ");
                                                buf.push_str(arg);
                                                buf.push('\n');
                                            }
                                        }
                                    }
                                    PseudoOps::PRINT | PseudoOps::READ => match tokens.next() {
                                        None => return Err(format!("Expected label at line {}", i + 1)),

                                        Some(arg) => {
                                            buf.push_str("IRQ ");
                                            buf.push_str((psop as u8).to_string().as_str());
                                            buf.push(' ');
                                            buf.push_str(arg);
                                            buf.push('\n');
                                        }
                                    }
                                    _ => {
                                        buf.push_str("IRQ ");
                                        buf.push_str((psop as u8).to_string().as_str());
                                        buf.push('\n');
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(token) = tokens.next() {
                    return Err(format!("Unexpected argument at line {}\n\t{}", i + 1, token));
                }
            } else {
                let mut tokens = line.split_whitespace();
                if let Some(token) = tokens.next() {
                    if let Ok(op) = OpCodes::from_str(token) {
                        if !began {
                            return Err(format!("Found instruction before BEGIN statement at line {}", i + 1));
                        }
                        match op {
                            OpCodes::IRQ => match tokens.next() {
                                None => return Err(format!("Expected integer at line {}", i + 1)),

                                Some(irq_type) => match irq_type.parse::<u8>() {
                                    Err(_) => return Err(format!("Expected integer at line {}\n\tfound {} instead", i + 1, irq_type)),

                                    Ok(v) => match tokens.next() {
                                        Some(arg) => match v {
                                            1..=3 => {
                                                buf.push_str("IRQ ");
                                                buf.push_str(irq_type);
                                                buf.push(' ');
                                                buf.push_str(arg);
                                                buf.push('\n');
                                            }
                                            0 | 4 => return Err(format!("Unexpected argument at line {}\n\t{}", i + 1, token)),
                                            _ => return Err(format!("Unknown IRQ type at line {}\n\t{}", i + 1, irq_type)),
                                        }
                                        None => match v {
                                            0 | 4 => {
                                                buf.push_str("IRQ ");
                                                buf.push_str(irq_type);
                                                buf.push('\n');
                                            }
                                            1..=3 => return Err(format!("Expected label at line {}", i + 1)),
                                            _ => return Err(format!("Unknown IRQ type at line {}\n\t{}", i + 1, irq_type)),
                                        }
                                    }
                                }
                            }
                            _ => match tokens.next() {
                                None => return Err(format!("Expected label at line {}", i + 1)),

                                Some(arg) => {
                                    buf.push_str(token);
                                    buf.push(' ');
                                    buf.push_str(arg);
                                    buf.push('\n');
                                }
                            }
                        }
                    } else {
                        match PseudoOps::from_str(token) {
                            Err(_) => return Err(format!("Expected label or instruction at line {}\t\nfound {} instead", i + 1, token)),

                            Ok(psop) => match began {
                                true => match psop {
                                    PseudoOps::EXTERN => return Err(format!("Found EXTERN statement after BEGIN at line {}", i + 1)),

                                    PseudoOps::BEGIN => return Err(format!("Found repeated BEGIN statement at line {}", i + 1)),

                                    PseudoOps::END => ended = true,

                                    PseudoOps::SET => match tokens.next() {
                                        None => return Err(format!("Expected label at line {}", i + 1)),

                                        Some(arg) => match u32::from_str_radix(arg, 2) {
                                            Err(_) => return Err(format!("Expected binary number as argument at line {}\n\tfound {} instead", i + 1, token)),

                                            Ok(_) => {
                                                buf.push_str("IRQ 3 ");
                                                buf.push_str(arg);
                                                buf.push('\n');
                                            }
                                        }
                                    }
                                    PseudoOps::PRINT | PseudoOps::READ => match tokens.next() {
                                        None => return Err(format!("Expected label at line {}", i + 1)),

                                        Some(arg) => {
                                            buf.push_str("IRQ ");
                                            buf.push_str((psop as u8).to_string().as_str());
                                            buf.push(' ');
                                            buf.push_str(arg);
                                            buf.push('\n');
                                        }
                                    }
                                    _ => {
                                        buf.push_str("IRQ ");
                                        buf.push_str((psop as u8).to_string().as_str());
                                        buf.push('\n');
                                    }
                                }
                                false => match psop {
                                    PseudoOps::BEGIN => {
                                        offset = match u32::try_from(i + 1) {
                                            Err(_) => return Err("File too big!".to_owned()),

                                            Ok(v) => v,
                                        };
                                        began = true;
                                    }
                                    PseudoOps::EXTERN => match tokens.next() {
                                        None => return Err(format!("Expected label after EXTERN at line {}", i + 1)),

                                        Some(label) => {
                                            buf.push_str(label);
                                            buf.push('\n');
                                            header_len += 1;
                                        }
                                    }
                                    _ => return Err(format!("Expected BEGIN or EXTERN statement or label at line {}\n\tfound {} instead", i + 1, token))
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }) {
        Ok(()) => (),
        Err(why) => return Ok((false, why)),
    }

    if !ended {
        return Ok((false, "END statement missing".to_owned()));
    }

    buf.insert_str(0, (header_len - 1).to_string().as_str());

    match fs::write(breadcrumb.unwrap_or("a.bdc"), buf) {
        Ok(_) => Ok((true, "Assembly successful".to_owned())),
        Err(why) => Ok((false, why.to_string())),
    }
}
