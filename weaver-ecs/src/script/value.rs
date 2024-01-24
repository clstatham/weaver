use std::{
    fmt::{Debug, Display},
    ops::{Deref, DerefMut},
};

use crate::{component::Data, prelude::Entity, query::DynamicQuery};

use super::{interp::RuntimeEnv, parser::TypedIdent};

#[derive(Clone)]
pub enum Value {
    Void,
    Float(f32),
    Int(i64),
    Bool(bool),
    Data(Data),
    DataMut(Data),
    Entity(Entity),
    Query {
        query: DynamicQuery,
        typed_idents: Vec<TypedIdent>,
    },
    Vec3(glam::Vec3),
    Vec4(glam::Vec4),
    Mat4(glam::Mat4),
    Quat(glam::Quat),
    String(String),
}

impl Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Value::DataMut(data) = self {
            write!(f, "DataMut({:?})", data.type_name())
        } else if let Value::Data(data) = self {
            write!(f, "Data({:?})", data.type_name())
        } else {
            write!(f, "{:?}", self.type_name())
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Void => write!(f, "()"),
            Value::Float(float) => write!(f, "{}", float),
            Value::Int(int) => write!(f, "{}", int),
            Value::Bool(bool) => write!(f, "{}", bool),
            Value::Data(data) => {
                data.display(f);
                Ok(())
            }
            Value::DataMut(data) => {
                data.display(f);
                Ok(())
            }
            Value::Entity(entity) => write!(f, "{:?}", entity),
            Value::Query { typed_idents, .. } => {
                write!(f, "Query(")?;
                for (i, typed_ident) in typed_idents.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", typed_ident.name)?;
                }
                write!(f, ")")
            }
            Value::Vec3(vec) => write!(f, "{}", vec),
            Value::Vec4(vec) => write!(f, "{}", vec),
            Value::Mat4(mat) => write!(f, "{}", mat),
            Value::Quat(quat) => write!(f, "{}", quat),
            Value::String(string) => write!(f, "{}", string),
        }
    }
}

impl Value {
    pub fn type_name(&self) -> &str {
        match self {
            Value::Void => "()",
            Value::Float(_) => "f64",
            Value::Int(_) => "i64",
            Value::Bool(_) => "bool",
            Value::Data(data) => data.type_name(),
            Value::DataMut(data) => data.type_name(),
            Value::Entity(_) => "Entity",
            Value::Query { .. } => "DynamicQuery",
            Value::Vec3(_) => "Vec3",
            Value::Vec4(_) => "Vec4",
            Value::Mat4(_) => "Mat4",
            Value::Quat(_) => "Quat",
            Value::String(_) => "String",
        }
    }

    pub fn to_data(&self, env: &RuntimeEnv) -> anyhow::Result<Self> {
        let data = match self {
            // pass by reference
            Value::Data(data) => Value::Data(data.to_owned()),
            Value::DataMut(data) => Value::DataMut(data.to_owned()),
            Value::Int(int) => Value::Data(Data::new(*int, None, env.world.read().registry())),
            Value::Float(float) => {
                Value::Data(Data::new(*float, None, env.world.read().registry()))
            }
            Value::Bool(b) => Value::Data(Data::new(*b, None, env.world.read().registry())),
            Value::Void => anyhow::bail!("Cannot assign void value"),
            Value::Query {
                query,
                typed_idents,
            } => {
                let query = query.to_owned();
                let typed_idents = typed_idents.to_owned();
                Value::Query {
                    query,
                    typed_idents,
                }
            }
            Value::Entity(entity) => Value::Entity(*entity),
            Value::Vec3(vec3) => Value::Data(Data::new(*vec3, None, env.world.read().registry())),
            Value::Vec4(vec4) => Value::Data(Data::new(*vec4, None, env.world.read().registry())),
            Value::Quat(quat) => Value::Data(Data::new(*quat, None, env.world.read().registry())),
            Value::Mat4(mat4) => Value::Data(Data::new(*mat4, None, env.world.read().registry())),
            Value::String(string) => Value::Data(Data::new(
                string.to_owned(),
                None,
                env.world.read().registry(),
            )),
        };
        Ok(data)
    }

    pub fn infix(&mut self, op: &str, rhs: &Self) -> anyhow::Result<Self> {
        match (self, rhs) {
            (Value::Int(lhs), Value::Int(rhs)) => match op {
                "+" => Ok(Value::Int(*lhs + rhs)),
                "-" => Ok(Value::Int(*lhs - rhs)),
                "*" => Ok(Value::Int(*lhs * rhs)),
                "/" => Ok(Value::Int(*lhs / rhs)),
                "%" => Ok(Value::Int(*lhs % rhs)),
                ">" => Ok(Value::Bool(*lhs > *rhs)),
                "<" => Ok(Value::Bool(*lhs < *rhs)),
                ">=" => Ok(Value::Bool(*lhs >= *rhs)),
                "<=" => Ok(Value::Bool(*lhs <= *rhs)),
                "==" => Ok(Value::Bool(*lhs == *rhs)),
                "=" | "+=" | "-=" | "*=" | "/=" | "%=" => {
                    Err(anyhow::anyhow!("Invalid operator {} for integer types", op))
                }
                _ => Err(anyhow::anyhow!("Invalid operator {}", op)),
            },
            (Value::Float(lhs), Value::Float(rhs)) => match op {
                "+" => Ok(Value::Float(*lhs + rhs)),
                "-" => Ok(Value::Float(*lhs - rhs)),
                "*" => Ok(Value::Float(*lhs * rhs)),
                "/" => Ok(Value::Float(*lhs / rhs)),
                "%" => Ok(Value::Float(*lhs % rhs)),
                ">" => Ok(Value::Bool(*lhs > *rhs)),
                "<" => Ok(Value::Bool(*lhs < *rhs)),
                ">=" => Ok(Value::Bool(*lhs >= *rhs)),
                "<=" => Ok(Value::Bool(*lhs <= *rhs)),
                "==" => Ok(Value::Bool(*lhs == *rhs)),
                "=" | "+=" | "-=" | "*=" | "/=" | "%=" => {
                    Err(anyhow::anyhow!("Invalid operator {} for float types", op))
                }
                _ => Err(anyhow::anyhow!("Invalid operator {}", op)),
            },
            (Value::Data(lhs), Value::Data(rhs)) | (Value::Data(lhs), Value::DataMut(rhs)) => {
                match op {
                    "+" => Ok(Value::Data(lhs.add(rhs)?)),
                    "-" => Ok(Value::Data(lhs.sub(rhs)?)),
                    "*" => Ok(Value::Data(lhs.mul(rhs)?)),
                    "/" => Ok(Value::Data(lhs.div(rhs)?)),
                    "%" => Ok(Value::Data(lhs.rem(rhs)?)),
                    ">" => Ok(Value::Data(lhs.gt(rhs)?)),
                    "<" => Ok(Value::Data(lhs.lt(rhs)?)),
                    ">=" => Ok(Value::Data(lhs.ge(rhs)?)),
                    "<=" => Ok(Value::Data(lhs.le(rhs)?)),
                    "==" => Ok(Value::Data(lhs.eq(rhs)?)),
                    "&&" => Ok(Value::Data(lhs.and(rhs)?)),
                    "||" => Ok(Value::Data(lhs.or(rhs)?)),
                    "^" => Ok(Value::Data(lhs.xor(rhs)?)),
                    "=" | "+=" | "-=" | "*=" | "/=" | "%=" => Err(anyhow::anyhow!(
                        "Invalid operator {}: cannot assign to immutable variable {}",
                        op,
                        lhs.name()
                    )),
                    _ => Err(anyhow::anyhow!("Invalid operator {}", op)),
                }
            }
            (Value::Data(lhs), Value::Int(rhs)) => {
                let rhs = Data::new(*rhs, None, lhs.registry());
                match op {
                    "+" => Ok(Value::DataMut(lhs.add(&rhs)?)),
                    "-" => Ok(Value::DataMut(lhs.sub(&rhs)?)),
                    "*" => Ok(Value::DataMut(lhs.mul(&rhs)?)),
                    "/" => Ok(Value::DataMut(lhs.div(&rhs)?)),
                    "%" => Ok(Value::DataMut(lhs.rem(&rhs)?)),
                    ">" => Ok(Value::DataMut(lhs.gt(&rhs)?)),
                    "<" => Ok(Value::DataMut(lhs.lt(&rhs)?)),
                    ">=" => Ok(Value::DataMut(lhs.ge(&rhs)?)),
                    "<=" => Ok(Value::DataMut(lhs.le(&rhs)?)),
                    "==" => Ok(Value::DataMut(lhs.eq(&rhs)?)),
                    "&&" => Ok(Value::DataMut(lhs.and(&rhs)?)),
                    "||" => Ok(Value::DataMut(lhs.or(&rhs)?)),
                    "^" => Ok(Value::DataMut(lhs.xor(&rhs)?)),
                    "=" | "+=" | "-=" | "*=" | "/=" | "%=" => Err(anyhow::anyhow!(
                        "Invalid operator {}: cannot assign to immutable variable {}",
                        op,
                        lhs.name()
                    )),
                    _ => Err(anyhow::anyhow!("Invalid operator {}", op)),
                }
            }
            (Value::Int(lhs), Value::Data(rhs)) | (Value::Int(lhs), Value::DataMut(rhs)) => {
                let lhs = Data::new(*lhs, None, rhs.registry());
                match op {
                    "+" => Ok(Value::DataMut(lhs.add(rhs)?)),
                    "-" => Ok(Value::DataMut(lhs.sub(rhs)?)),
                    "*" => Ok(Value::DataMut(lhs.mul(rhs)?)),
                    "/" => Ok(Value::DataMut(lhs.div(rhs)?)),
                    "%" => Ok(Value::DataMut(lhs.rem(rhs)?)),
                    ">" => Ok(Value::DataMut(lhs.gt(rhs)?)),
                    "<" => Ok(Value::DataMut(lhs.lt(rhs)?)),
                    ">=" => Ok(Value::DataMut(lhs.ge(rhs)?)),
                    "<=" => Ok(Value::DataMut(lhs.le(rhs)?)),
                    "==" => Ok(Value::DataMut(lhs.eq(rhs)?)),
                    "&&" => Ok(Value::DataMut(lhs.and(rhs)?)),
                    "||" => Ok(Value::DataMut(lhs.or(rhs)?)),
                    "^" => Ok(Value::DataMut(lhs.xor(rhs)?)),
                    "=" | "+=" | "-=" | "*=" | "/=" | "%=" => {
                        Err(anyhow::anyhow!("Invalid operator {} for integer types", op,))
                    }
                    _ => Err(anyhow::anyhow!("Invalid operator {}", op)),
                }
            }
            (Value::DataMut(lhs), Value::Int(rhs)) => {
                let rhs = Data::new(*rhs, None, lhs.registry());
                match op {
                    "+" => Ok(Value::DataMut(lhs.add(&rhs)?)),
                    "-" => Ok(Value::DataMut(lhs.sub(&rhs)?)),
                    "*" => Ok(Value::DataMut(lhs.mul(&rhs)?)),
                    "/" => Ok(Value::DataMut(lhs.div(&rhs)?)),
                    "%" => Ok(Value::DataMut(lhs.rem(&rhs)?)),
                    ">" => Ok(Value::DataMut(lhs.gt(&rhs)?)),
                    "<" => Ok(Value::DataMut(lhs.lt(&rhs)?)),
                    ">=" => Ok(Value::DataMut(lhs.ge(&rhs)?)),
                    "<=" => Ok(Value::DataMut(lhs.le(&rhs)?)),
                    "==" => Ok(Value::DataMut(lhs.eq(&rhs)?)),
                    "&&" => Ok(Value::DataMut(lhs.and(&rhs)?)),
                    "||" => Ok(Value::DataMut(lhs.or(&rhs)?)),
                    "^" => Ok(Value::DataMut(lhs.xor(&rhs)?)),
                    "=" => {
                        lhs.assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    "+=" => {
                        lhs.add_assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    "-=" => {
                        lhs.sub_assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    "*=" => {
                        lhs.mul_assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    "/=" => {
                        lhs.div_assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    "%=" => {
                        lhs.rem_assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    _ => Err(anyhow::anyhow!("Invalid operator {}", op)),
                }
            }
            (Value::Data(lhs), Value::Float(rhs)) => {
                let rhs = Data::new(*rhs, None, lhs.registry());
                match op {
                    "+" => Ok(Value::DataMut(lhs.add(&rhs)?)),
                    "-" => Ok(Value::DataMut(lhs.sub(&rhs)?)),
                    "*" => Ok(Value::DataMut(lhs.mul(&rhs)?)),
                    "/" => Ok(Value::DataMut(lhs.div(&rhs)?)),
                    "%" => Ok(Value::DataMut(lhs.rem(&rhs)?)),
                    ">" => Ok(Value::DataMut(lhs.gt(&rhs)?)),
                    "<" => Ok(Value::DataMut(lhs.lt(&rhs)?)),
                    ">=" => Ok(Value::DataMut(lhs.ge(&rhs)?)),
                    "<=" => Ok(Value::DataMut(lhs.le(&rhs)?)),
                    "==" => Ok(Value::DataMut(lhs.eq(&rhs)?)),
                    "&&" => Ok(Value::DataMut(lhs.and(&rhs)?)),
                    "||" => Ok(Value::DataMut(lhs.or(&rhs)?)),
                    "^" => Ok(Value::DataMut(lhs.xor(&rhs)?)),
                    "=" | "+=" | "-=" | "*=" | "/=" | "%=" => Err(anyhow::anyhow!(
                        "Invalid operator {}: cannot assign to immutable variable {}",
                        op,
                        lhs.name()
                    )),
                    _ => Err(anyhow::anyhow!("Invalid operator {}", op)),
                }
            }
            (Value::Float(lhs), Value::Data(rhs)) | (Value::Float(lhs), Value::DataMut(rhs)) => {
                let lhs = Data::new(*lhs, None, rhs.registry());
                match op {
                    "+" => Ok(Value::DataMut(lhs.add(rhs)?)),
                    "-" => Ok(Value::DataMut(lhs.sub(rhs)?)),
                    "*" => Ok(Value::DataMut(lhs.mul(rhs)?)),
                    "/" => Ok(Value::DataMut(lhs.div(rhs)?)),
                    "%" => Ok(Value::DataMut(lhs.rem(rhs)?)),
                    ">" => Ok(Value::DataMut(lhs.gt(rhs)?)),
                    "<" => Ok(Value::DataMut(lhs.lt(rhs)?)),
                    ">=" => Ok(Value::DataMut(lhs.ge(rhs)?)),
                    "<=" => Ok(Value::DataMut(lhs.le(rhs)?)),
                    "==" => Ok(Value::DataMut(lhs.eq(rhs)?)),
                    "&&" => Ok(Value::DataMut(lhs.and(rhs)?)),
                    "||" => Ok(Value::DataMut(lhs.or(rhs)?)),
                    "^" => Ok(Value::DataMut(lhs.xor(rhs)?)),
                    "=" | "+=" | "-=" | "*=" | "/=" | "%=" => {
                        Err(anyhow::anyhow!("Invalid operator {} for float types", op,))
                    }
                    _ => Err(anyhow::anyhow!("Invalid operator {}", op)),
                }
            }
            (Value::DataMut(lhs), Value::Float(rhs)) => {
                let rhs = Data::new(*rhs, None, lhs.registry());
                match op {
                    "+" => Ok(Value::DataMut(lhs.add(&rhs)?)),
                    "-" => Ok(Value::DataMut(lhs.sub(&rhs)?)),
                    "*" => Ok(Value::DataMut(lhs.mul(&rhs)?)),
                    "/" => Ok(Value::DataMut(lhs.div(&rhs)?)),
                    "%" => Ok(Value::DataMut(lhs.rem(&rhs)?)),
                    ">" => Ok(Value::DataMut(lhs.gt(&rhs)?)),
                    "<" => Ok(Value::DataMut(lhs.lt(&rhs)?)),
                    ">=" => Ok(Value::DataMut(lhs.ge(&rhs)?)),
                    "<=" => Ok(Value::DataMut(lhs.le(&rhs)?)),
                    "==" => Ok(Value::DataMut(lhs.eq(&rhs)?)),
                    "&&" => Ok(Value::DataMut(lhs.and(&rhs)?)),
                    "||" => Ok(Value::DataMut(lhs.or(&rhs)?)),
                    "^" => Ok(Value::DataMut(lhs.xor(&rhs)?)),
                    "=" => {
                        lhs.assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    "+=" => {
                        lhs.add_assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    "-=" => {
                        lhs.sub_assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    "*=" => {
                        lhs.mul_assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    "/=" => {
                        lhs.div_assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    "%=" => {
                        lhs.rem_assign(&rhs)?;
                        Ok(Value::Void)
                    }
                    _ => Err(anyhow::anyhow!("Invalid operator {}", op)),
                }
            }
            (Value::DataMut(lhs), Value::Data(rhs))
            | (Value::DataMut(lhs), Value::DataMut(rhs)) => match op {
                "+" => Ok(Value::DataMut(lhs.add(rhs)?)),
                "-" => Ok(Value::DataMut(lhs.sub(rhs)?)),
                "*" => Ok(Value::DataMut(lhs.mul(rhs)?)),
                "/" => Ok(Value::DataMut(lhs.div(rhs)?)),
                "%" => Ok(Value::DataMut(lhs.rem(rhs)?)),
                ">" => Ok(Value::DataMut(lhs.gt(rhs)?)),
                "<" => Ok(Value::DataMut(lhs.lt(rhs)?)),
                ">=" => Ok(Value::DataMut(lhs.ge(rhs)?)),
                "<=" => Ok(Value::DataMut(lhs.le(rhs)?)),
                "==" => Ok(Value::DataMut(lhs.eq(rhs)?)),
                "&&" => Ok(Value::DataMut(lhs.and(rhs)?)),
                "||" => Ok(Value::DataMut(lhs.or(rhs)?)),
                "^" => Ok(Value::DataMut(lhs.xor(rhs)?)),
                "=" => {
                    lhs.assign(rhs)?;
                    Ok(Value::Void)
                }
                "+=" => {
                    lhs.add_assign(rhs)?;
                    Ok(Value::Void)
                }
                "-=" => {
                    lhs.sub_assign(rhs)?;
                    Ok(Value::Void)
                }
                "*=" => {
                    lhs.mul_assign(rhs)?;
                    Ok(Value::Void)
                }
                "/=" => {
                    lhs.div_assign(rhs)?;
                    Ok(Value::Void)
                }
                "%=" => {
                    lhs.rem_assign(rhs)?;
                    Ok(Value::Void)
                }
                _ => Err(anyhow::anyhow!("Invalid operator {}", op)),
            },
            (Value::Bool(lhs), Value::Bool(rhs)) => match op {
                "&&" => Ok(Value::Bool(*lhs && *rhs)),
                "||" => Ok(Value::Bool(*lhs || *rhs)),
                "^" => Ok(Value::Bool(*lhs ^ *rhs)),
                "=" | "+=" | "-=" | "*=" | "/=" | "%=" => {
                    Err(anyhow::anyhow!("Invalid operator {} for boolean types", op))
                }
                _ => Err(anyhow::anyhow!("Invalid operator {}", op)),
            },
            (Value::Vec3(lhs), Value::Vec3(rhs)) => match op {
                "+" => Ok(Value::Vec3(*lhs + *rhs)),
                "-" => Ok(Value::Vec3(*lhs - *rhs)),
                "*" => Ok(Value::Vec3(*lhs * *rhs)),
                "/" => Ok(Value::Vec3(*lhs / *rhs)),
                "%" => Ok(Value::Vec3(*lhs % *rhs)),
                "=" => {
                    *lhs = *rhs;
                    Ok(Value::Void)
                }
                "+=" => {
                    *lhs += *rhs;
                    Ok(Value::Void)
                }
                "-=" => {
                    *lhs -= *rhs;
                    Ok(Value::Void)
                }
                "*=" => {
                    *lhs *= *rhs;
                    Ok(Value::Void)
                }
                "/=" => {
                    *lhs /= *rhs;
                    Ok(Value::Void)
                }
                "%=" => {
                    *lhs %= *rhs;
                    Ok(Value::Void)
                }
                _ => Err(anyhow::anyhow!("Invalid operator {} for Vec3 types", op)),
            },
            (Value::Vec4(lhs), Value::Vec4(rhs)) => match op {
                "+" => Ok(Value::Vec4(*lhs + *rhs)),
                "-" => Ok(Value::Vec4(*lhs - *rhs)),
                "*" => Ok(Value::Vec4(*lhs * *rhs)),
                "/" => Ok(Value::Vec4(*lhs / *rhs)),
                "%" => Ok(Value::Vec4(*lhs % *rhs)),
                "=" => {
                    *lhs = *rhs;
                    Ok(Value::Void)
                }
                "+=" => {
                    *lhs += *rhs;
                    Ok(Value::Void)
                }
                "-=" => {
                    *lhs -= *rhs;
                    Ok(Value::Void)
                }
                "*=" => {
                    *lhs *= *rhs;
                    Ok(Value::Void)
                }
                "/=" => {
                    *lhs /= *rhs;
                    Ok(Value::Void)
                }
                "%=" => {
                    *lhs %= *rhs;
                    Ok(Value::Void)
                }
                "==" => Ok(Value::Bool(*lhs == *rhs)),
                _ => Err(anyhow::anyhow!("Invalid operator {} for Vec4 types", op)),
            },
            (Value::Mat4(lhs), Value::Mat4(rhs)) => match op {
                "+" => Ok(Value::Mat4(*lhs + *rhs)),
                "-" => Ok(Value::Mat4(*lhs - *rhs)),
                "*" => Ok(Value::Mat4(*lhs * *rhs)),
                "=" => {
                    *lhs = *rhs;
                    Ok(Value::Void)
                }
                "+=" => {
                    *lhs += *rhs;
                    Ok(Value::Void)
                }
                "-=" => {
                    *lhs -= *rhs;
                    Ok(Value::Void)
                }
                "*=" => {
                    *lhs *= *rhs;
                    Ok(Value::Void)
                }
                "==" => Ok(Value::Bool(*lhs == *rhs)),
                _ => Err(anyhow::anyhow!("Invalid operator {} for Mat4 types", op)),
            },
            (Value::Quat(lhs), Value::Quat(rhs)) => match op {
                "+" => Ok(Value::Quat(*lhs + *rhs)),
                "-" => Ok(Value::Quat(*lhs - *rhs)),
                "*" => Ok(Value::Quat(*lhs * *rhs)),
                "=" => {
                    *lhs = *rhs;
                    Ok(Value::Void)
                }
                _ => Err(anyhow::anyhow!("Invalid operator {} for Quat types", op)),
            },
            (Value::Vec3(lhs), Value::Float(rhs)) => match op {
                "+" => Ok(Value::Vec3(*lhs + *rhs)),
                "-" => Ok(Value::Vec3(*lhs - *rhs)),
                "*" => Ok(Value::Vec3(*lhs * *rhs)),
                "/" => Ok(Value::Vec3(*lhs / *rhs)),
                "%" => Ok(Value::Vec3(*lhs % *rhs)),
                "=" => {
                    *lhs = glam::Vec3::splat(*rhs);
                    Ok(Value::Void)
                }
                "+=" => {
                    *lhs += glam::Vec3::splat(*rhs);
                    Ok(Value::Void)
                }
                "-=" => {
                    *lhs -= glam::Vec3::splat(*rhs);
                    Ok(Value::Void)
                }
                "*=" => {
                    *lhs *= glam::Vec3::splat(*rhs);
                    Ok(Value::Void)
                }
                "/=" => {
                    *lhs /= glam::Vec3::splat(*rhs);
                    Ok(Value::Void)
                }
                "%=" => {
                    *lhs %= glam::Vec3::splat(*rhs);
                    Ok(Value::Void)
                }
                _ => Err(anyhow::anyhow!("Invalid operator {} for Vec3 types", op)),
            },
            (Value::Vec4(lhs), Value::Float(rhs)) => match op {
                "+" => Ok(Value::Vec4(*lhs + *rhs)),
                "-" => Ok(Value::Vec4(*lhs - *rhs)),
                "*" => Ok(Value::Vec4(*lhs * *rhs)),
                "/" => Ok(Value::Vec4(*lhs / *rhs)),
                "%" => Ok(Value::Vec4(*lhs % *rhs)),
                "=" => {
                    *lhs = glam::Vec4::splat(*rhs);
                    Ok(Value::Void)
                }
                "+=" => {
                    *lhs += glam::Vec4::splat(*rhs);
                    Ok(Value::Void)
                }
                "-=" => {
                    *lhs -= glam::Vec4::splat(*rhs);
                    Ok(Value::Void)
                }
                "*=" => {
                    *lhs *= glam::Vec4::splat(*rhs);
                    Ok(Value::Void)
                }
                "/=" => {
                    *lhs /= glam::Vec4::splat(*rhs);
                    Ok(Value::Void)
                }
                "%=" => {
                    *lhs %= glam::Vec4::splat(*rhs);
                    Ok(Value::Void)
                }
                _ => Err(anyhow::anyhow!("Invalid operator {} for Vec4 types", op)),
            },
            (Value::Mat4(lhs), Value::Float(rhs)) => match op {
                "*" => Ok(Value::Mat4(*lhs * *rhs)),
                "*=" => {
                    *lhs *= *rhs;
                    Ok(Value::Void)
                }
                _ => Err(anyhow::anyhow!("Invalid operator {} for Mat4 types", op)),
            },
            (Value::Quat(lhs), Value::Float(rhs)) => match op {
                "*" => Ok(Value::Quat(*lhs * *rhs)),
                "/" => Ok(Value::Quat(*lhs / *rhs)),
                _ => Err(anyhow::anyhow!("Invalid operator {} for Quat types", op)),
            },
            (Value::Float(lhs), Value::Vec3(rhs)) => match op {
                "+" => Ok(Value::Vec3(*lhs + *rhs)),
                "-" => Ok(Value::Vec3(*lhs - *rhs)),
                "*" => Ok(Value::Vec3(*lhs * *rhs)),
                "/" => Ok(Value::Vec3(*lhs / *rhs)),
                "%" => Ok(Value::Vec3(*lhs % *rhs)),
                _ => Err(anyhow::anyhow!("Invalid operator {} for Vec3 types", op)),
            },
            (Value::Float(lhs), Value::Vec4(rhs)) => match op {
                "+" => Ok(Value::Vec4(*lhs + *rhs)),
                "-" => Ok(Value::Vec4(*lhs - *rhs)),
                "*" => Ok(Value::Vec4(*lhs * *rhs)),
                "/" => Ok(Value::Vec4(*lhs / *rhs)),
                "%" => Ok(Value::Vec4(*lhs % *rhs)),
                _ => Err(anyhow::anyhow!("Invalid operator {} for Vec4 types", op)),
            },
            (Value::Float(lhs), Value::Mat4(rhs)) => match op {
                "*" => Ok(Value::Mat4(*lhs * *rhs)),
                _ => Err(anyhow::anyhow!("Invalid operator {} for Mat4 types", op)),
            },
            (Value::Data(lhs), Value::Vec3(rhs)) => {
                let lhs = lhs.get_as_mut::<glam::Vec3>();
                match op {
                    "+" => Ok(Value::Vec3(*lhs + *rhs)),
                    "-" => Ok(Value::Vec3(*lhs - *rhs)),
                    "*" => Ok(Value::Vec3(*lhs * *rhs)),
                    "/" => Ok(Value::Vec3(*lhs / *rhs)),
                    "%" => Ok(Value::Vec3(*lhs % *rhs)),
                    "==" => Ok(Value::Bool(*lhs == *rhs)),
                    _ => Err(anyhow::anyhow!("Invalid operator {} for Vec3 types", op)),
                }
            }
            (Value::Data(lhs), Value::Vec4(rhs)) => {
                let lhs = lhs.get_as_mut::<glam::Vec4>();
                match op {
                    "+" => Ok(Value::Vec4(*lhs + *rhs)),
                    "-" => Ok(Value::Vec4(*lhs - *rhs)),
                    "*" => Ok(Value::Vec4(*lhs * *rhs)),
                    "/" => Ok(Value::Vec4(*lhs / *rhs)),
                    "%" => Ok(Value::Vec4(*lhs % *rhs)),
                    "==" => Ok(Value::Bool(*lhs == *rhs)),
                    _ => Err(anyhow::anyhow!("Invalid operator {} for Vec4 types", op)),
                }
            }
            (Value::Data(lhs), Value::Mat4(rhs)) => {
                let lhs = lhs.get_as_mut::<glam::Mat4>();
                match op {
                    "*" => Ok(Value::Mat4(*lhs * *rhs)),
                    "==" => Ok(Value::Bool(*lhs == *rhs)),
                    _ => Err(anyhow::anyhow!("Invalid operator {} for Mat4 types", op)),
                }
            }
            (Value::Data(lhs), Value::Quat(rhs)) => {
                let lhs = lhs.get_as_mut::<glam::Quat>();
                match op {
                    "*" => Ok(Value::Quat(*lhs * *rhs)),
                    "==" => Ok(Value::Bool(*lhs == *rhs)),
                    _ => Err(anyhow::anyhow!("Invalid operator {} for Quat types", op)),
                }
            }
            (Value::Vec3(lhs), Value::Data(rhs) | Value::DataMut(rhs)) => {
                let rhs = rhs.get_as::<glam::Vec3>();
                match op {
                    "+" => Ok(Value::Vec3(*lhs + *rhs)),
                    "-" => Ok(Value::Vec3(*lhs - *rhs)),
                    "*" => Ok(Value::Vec3(*lhs * *rhs)),
                    "/" => Ok(Value::Vec3(*lhs / *rhs)),
                    "%" => Ok(Value::Vec3(*lhs % *rhs)),
                    "==" => Ok(Value::Bool(*lhs == *rhs)),
                    _ => Err(anyhow::anyhow!("Invalid operator {} for Vec3 types", op)),
                }
            }
            (Value::Vec4(lhs), Value::Data(rhs) | Value::DataMut(rhs)) => {
                let rhs = rhs.get_as::<glam::Vec4>();
                match op {
                    "+" => Ok(Value::Vec4(*lhs + *rhs)),
                    "-" => Ok(Value::Vec4(*lhs - *rhs)),
                    "*" => Ok(Value::Vec4(*lhs * *rhs)),
                    "/" => Ok(Value::Vec4(*lhs / *rhs)),
                    "%" => Ok(Value::Vec4(*lhs % *rhs)),
                    "==" => Ok(Value::Bool(*lhs == *rhs)),
                    _ => Err(anyhow::anyhow!("Invalid operator {} for Vec4 types", op)),
                }
            }
            (Value::Mat4(lhs), Value::Data(rhs) | Value::DataMut(rhs)) => {
                let rhs = rhs.get_as::<glam::Mat4>();
                match op {
                    "*" => Ok(Value::Mat4(*lhs * *rhs)),
                    "==" => Ok(Value::Bool(*lhs == *rhs)),
                    _ => Err(anyhow::anyhow!("Invalid operator {} for Mat4 types", op)),
                }
            }
            (Value::Quat(lhs), Value::Data(rhs) | Value::DataMut(rhs)) => {
                let rhs = rhs.get_as::<glam::Quat>();
                match op {
                    "*" => Ok(Value::Quat(*lhs * *rhs)),
                    "==" => Ok(Value::Bool(*lhs == *rhs)),
                    _ => Err(anyhow::anyhow!("Invalid operator {} for Quat types", op)),
                }
            }
            (Value::DataMut(lhs), Value::Vec3(rhs)) => {
                let mut lhs = lhs.get_as_mut::<glam::Vec3>();
                match op {
                    "+" => Ok(Value::Vec3(*lhs + *rhs)),
                    "-" => Ok(Value::Vec3(*lhs - *rhs)),
                    "*" => Ok(Value::Vec3(*lhs * *rhs)),
                    "/" => Ok(Value::Vec3(*lhs / *rhs)),
                    "%" => Ok(Value::Vec3(*lhs % *rhs)),
                    "=" => {
                        *lhs = *rhs;
                        Ok(Value::Void)
                    }
                    "+=" => {
                        *lhs += *rhs;
                        Ok(Value::Void)
                    }
                    "-=" => {
                        *lhs -= *rhs;
                        Ok(Value::Void)
                    }
                    "*=" => {
                        *lhs *= *rhs;
                        Ok(Value::Void)
                    }
                    "/=" => {
                        *lhs /= *rhs;
                        Ok(Value::Void)
                    }
                    "%=" => {
                        *lhs %= *rhs;
                        Ok(Value::Void)
                    }
                    _ => Err(anyhow::anyhow!("Invalid operator {} for Vec3 types", op)),
                }
            }
            (Value::DataMut(lhs), Value::Vec4(rhs)) => {
                let mut lhs = lhs.get_as_mut::<glam::Vec4>();
                match op {
                    "+" => Ok(Value::Vec4(*lhs + *rhs)),
                    "-" => Ok(Value::Vec4(*lhs - *rhs)),
                    "*" => Ok(Value::Vec4(*lhs * *rhs)),
                    "/" => Ok(Value::Vec4(*lhs / *rhs)),
                    "%" => Ok(Value::Vec4(*lhs % *rhs)),
                    "=" => {
                        *lhs = *rhs;
                        Ok(Value::Void)
                    }
                    "+=" => {
                        *lhs += *rhs;
                        Ok(Value::Void)
                    }
                    "-=" => {
                        *lhs -= *rhs;
                        Ok(Value::Void)
                    }
                    "*=" => {
                        *lhs *= *rhs;
                        Ok(Value::Void)
                    }
                    "/=" => {
                        *lhs /= *rhs;
                        Ok(Value::Void)
                    }
                    "%=" => {
                        *lhs %= *rhs;
                        Ok(Value::Void)
                    }
                    _ => Err(anyhow::anyhow!("Invalid operator {} for Vec4 types", op)),
                }
            }
            (Value::DataMut(lhs), Value::Mat4(rhs)) => {
                let mut lhs = lhs.get_as_mut::<glam::Mat4>();
                match op {
                    "*" => Ok(Value::Mat4(*lhs * *rhs)),
                    "=" => {
                        *lhs = *rhs;
                        Ok(Value::Void)
                    }
                    "+=" => {
                        *lhs += *rhs;
                        Ok(Value::Void)
                    }
                    "-=" => {
                        *lhs -= *rhs;
                        Ok(Value::Void)
                    }
                    "*=" => {
                        *lhs *= *rhs;
                        Ok(Value::Void)
                    }
                    _ => Err(anyhow::anyhow!("Invalid operator {} for Mat4 types", op)),
                }
            }
            (Value::DataMut(lhs), Value::Quat(rhs)) => {
                let mut lhs = lhs.get_as_mut::<glam::Quat>();
                match op {
                    "*" => Ok(Value::Quat(*lhs * *rhs)),
                    "=" => {
                        *lhs = *rhs;
                        Ok(Value::Void)
                    }
                    "*=" => {
                        *lhs *= *rhs;
                        Ok(Value::Void)
                    }
                    _ => Err(anyhow::anyhow!("Invalid operator {} for Quat types", op)),
                }
            }
            (lhs, rhs) => Err(anyhow::anyhow!(
                "Invalid operator {} for {:?} and {:?}",
                op,
                lhs,
                rhs
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ValueHandle {
    pub name: Option<String>,
    pub id: usize,
    pub value: Value,
}

impl Deref for ValueHandle {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl DerefMut for ValueHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}
