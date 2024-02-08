use anyhow::{bail, Result};

use crate::{
    prelude::{Data, Entity, Read, SharedLock},
    query::QueryBuilderAccess,
};

use super::interp::RuntimeEnv;

macro_rules! try_all_types {
    ($s:expr; $($typ:ty),*; |$data:ident| $body:block else $els:block) => {{
        $(
            if let Some($data) = $s.as_ref::<$typ>() {
                #[allow(clippy::needless_return)]
                $body
            }
        )*
        #[allow(clippy::needless_return)]
        $els
    }};
}

macro_rules! try_all_types_both {
    ($s:expr, $o:expr; $($typ:ty),*; |$lhs:ident, $rhs:ident| $body:block else $els:block) => {{
        $(
            if let Some($lhs) = $s.as_ref::<$typ>() {
                if let Some($rhs) = $o.as_ref::<$typ>() {
                    #[allow(clippy::needless_return)]
                    $body
                }
            }
        )*
        #[allow(clippy::needless_return)]
        $els
    }};
}

macro_rules! try_all_types_both_mut_lhs {
    ($s:expr, $o:expr; $($typ:ty),*; |$lhs:ident, $rhs:ident| $body:block else $els:block) => {{
        $(
            if let Some($lhs) = $s.as_mut::<$typ>() {
                if let Some($rhs) = $o.as_ref::<$typ>() {
                    #[allow(clippy::needless_return)]
                    $body
                }
            }
        )*
        #[allow(clippy::needless_return)]
        $els
    }};
}

#[derive(Debug)]
pub enum Value {
    Void,
    Bool(bool),
    Int(i64),
    Float(f32),
    String(String),
    Data(Data),
    Resource(Entity),
    Query(Vec<(String, QueryBuilderAccess)>),
}

impl Value {
    pub fn display(&self, _env: &RuntimeEnv) -> String {
        match self {
            Value::Void => "Void".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::String(s) => s.clone(),
            Value::Data(r) => {
                if let Some(s) = self.as_float() {
                    s.to_string()
                } else if let Some(s) = self.as_int() {
                    s.to_string()
                } else {
                    format!("Data({:?})", r)
                }
            }
            Value::Resource(r) => format!("Res({:?})", r),
            Value::Query(q) => format!("Query({:?})", q),
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f32> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Data(d) => {
                try_all_types!(d; f32; |data| {
                    return Some(*data);
                } else {
                    None
                })
            }
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_data(&self) -> Option<&Data> {
        match self {
            Value::Data(d) => Some(d),
            _ => None,
        }
    }

    pub fn as_resource(&self) -> Option<Entity> {
        match self {
            Value::Resource(r) => Some(*r),
            _ => None,
        }
    }

    pub fn value_uid(&self, env: &RuntimeEnv) -> Result<Entity> {
        match self {
            Value::Data(d) => Ok(*d.value_uid()),
            Value::Bool(b) => {
                let data = env.world.write().create_data(*b)?;
                Ok(data)
            }
            Value::Int(i) => {
                let data = env.world.write().create_data(*i)?;
                Ok(data)
            }
            Value::Float(f) => {
                let data = env.world.write().create_data(*f)?;
                Ok(data)
            }
            Value::String(s) => {
                let data = env.world.write().create_data(s.to_owned())?;
                Ok(data)
            }
            Value::Resource(_) => bail!("Cannot convert Resource to Data"),
            Value::Query(_) => bail!("Cannot convert Query to Data"),
            Value::Void => bail!("Cannot convert Void to Data"),
        }
    }

    pub fn to_owned(&self) -> Value {
        match self {
            Value::Data(d) => Value::Data(d.to_owned()),
            Value::Bool(b) => Value::Bool(*b),
            Value::Int(i) => Value::Int(*i),
            Value::Float(f) => Value::Float(*f),
            Value::String(s) => Value::String(s.clone()),
            Value::Resource(r) => Value::Resource(*r),
            Value::Query(q) => Value::Query(q.clone()),
            Value::Void => Value::Void,
        }
    }

    pub fn to_owned_data(&self) -> Result<Data> {
        match self {
            Value::Data(d) => Ok(d.to_owned()),
            Value::Bool(b) => Ok(Data::new_dynamic(*b)),
            Value::Int(i) => Ok(Data::new_dynamic(*i)),
            Value::Float(f) => Ok(Data::new_dynamic(*f)),
            Value::String(s) => Ok(Data::new_dynamic(s.clone())),
            Value::Resource(_) => bail!("Cannot convert Resource to Data"),
            Value::Query(_) => bail!("Cannot convert Query to Data"),
            Value::Void => bail!("Cannot convert Void to Data"),
        }
    }

    pub fn despawn(&self) {
        if let Value::Data(d) = self {
            if d.as_dynamic().is_some() {
                d.value_uid().kill();
            }
        }
    }

    pub fn try_downcast_data(&self) -> Option<Self> {
        if let Value::Data(d) = self {
            if let Some(b) = d.as_ref::<bool>() {
                return Some(Value::Bool(*b));
            } else if let Some(i) = d.as_ref::<isize>() {
                return Some(Value::Int(*i as i64));
            } else if let Some(i) = d.as_ref::<i64>() {
                return Some(Value::Int(*i));
            } else if let Some(i) = d.as_ref::<i32>() {
                return Some(Value::Int(*i as i64));
            } else if let Some(i) = d.as_ref::<i16>() {
                return Some(Value::Int(*i as i64));
            } else if let Some(i) = d.as_ref::<i8>() {
                return Some(Value::Int(*i as i64));
            } else if let Some(i) = d.as_ref::<usize>() {
                return Some(Value::Int(*i as i64));
            } else if let Some(i) = d.as_ref::<u64>() {
                return Some(Value::Int(*i as i64));
            } else if let Some(i) = d.as_ref::<u32>() {
                return Some(Value::Int(*i as i64));
            } else if let Some(i) = d.as_ref::<u16>() {
                return Some(Value::Int(*i as i64));
            } else if let Some(i) = d.as_ref::<u8>() {
                return Some(Value::Int(*i as i64));
            } else if let Some(f) = d.as_ref::<f32>() {
                return Some(Value::Float(*f));
            } else if let Some(f) = d.as_ref::<f64>() {
                return Some(Value::Float(*f as f32));
            } else if let Some(s) = d.as_ref::<String>() {
                return Some(Value::String(s.clone()));
            }
        }
        None
    }

    pub fn prefix(&self, op: &str) -> Result<Value> {
        match self {
            Value::Bool(a) => match op {
                "!" => Ok(Value::Bool(!a)),
                _ => bail!("Invalid prefix operator for boolean: {}", op),
            },
            Value::Int(a) => match op {
                "-" => Ok(Value::Int(-a)),
                _ => bail!("Invalid prefix operator for integer: {}", op),
            },
            Value::Float(a) => match op {
                "-" => Ok(Value::Float(-a)),
                _ => bail!("Invalid prefix operator for float: {}", op),
            },
            Value::Data(a) => match op {
                "-" => {
                    try_all_types!(a; i8, i16, i32, i64, i128, isize, f32, f64; |data| {
                        return Ok(Value::Data(Data::new_dynamic(-data)));
                    } else {
                        bail!("Invalid data type for operator `-`: {:?}", a.type_uid().type_name());
                    })
                }
                _ => bail!("Invalid prefix operator for data: {}", op),
            },
            _ => bail!("Invalid prefix operator for value: {}", op),
        }
    }

    #[allow(clippy::needless_return)]
    pub fn infix(&self, op: &str, other: &Value, env: &RuntimeEnv) -> Result<Value> {
        match (self, other) {
            (Value::Bool(a), Value::Bool(b)) => match op {
                "&&" => Ok(Value::Bool(*a && *b)),
                "||" => Ok(Value::Bool(*a || *b)),
                "^" => Ok(Value::Bool(*a ^ *b)),
                _ => bail!("Invalid infix operator for boolean: {}", op),
            },
            (Value::Int(a), Value::Int(b)) => match op {
                "+" => Ok(Value::Int(a + b)),
                "-" => Ok(Value::Int(a - b)),
                "*" => Ok(Value::Int(a * b)),
                "/" => Ok(Value::Int(a / b)),
                "%" => Ok(Value::Int(a % b)),
                _ => bail!("Invalid infix operator for integer: {}", op),
            },
            (Value::Float(a), Value::Float(b)) => match op {
                "+" => Ok(Value::Float(a + b)),
                "-" => Ok(Value::Float(a - b)),
                "*" => Ok(Value::Float(a * b)),
                "/" => Ok(Value::Float(a / b)),
                "%" => Ok(Value::Float(a % b)),
                _ => bail!("Invalid infix operator for float: {}", op),
            },
            (Value::Data(_), Value::Int(b)) => {
                let b = Value::Data(Data::new_dynamic(*b));
                self.infix(op, &b, env)
            }
            (Value::Int(a), Value::Data(_)) => {
                let a = Value::Data(Data::new_dynamic(*a));
                a.infix(op, other, env)
            }
            (Value::Data(_), Value::Float(b)) => {
                let b = Value::Data(Data::new_dynamic(*b));
                self.infix(op, &b, env)
            }
            (Value::Float(a), Value::Data(_)) => {
                let a = Value::Data(Data::new_dynamic(*a));
                a.infix(op, other, env)
            }
            (Value::Data(a), Value::Data(b)) => {
                if let Some(b) = b.as_pointer() {
                    // deref
                    let world = env.world.read();
                    let b = world
                        .storage()
                        .find(b.target_type_uid(), b.target_value_uid())
                        .ok_or_else(|| {
                            anyhow::anyhow!("Could not find data: {}", b.target_value_uid())
                        })?;

                    let b = b.to_owned().unwrap();
                    let b = Value::Data(b);
                    return self.infix(op, &b, env);
                }
                if let Some(a) = a.as_pointer() {
                    // deref
                    let world = env.world.read();
                    let a = world
                        .storage()
                        .find(a.target_type_uid(), a.target_value_uid())
                        .ok_or_else(|| {
                            anyhow::anyhow!("Could not find data: {}", a.target_value_uid())
                        })?;

                    let a = a.to_owned().unwrap();
                    let a = Value::Data(a);
                    return a.infix(op, other, env);
                }
                if let Some(a) = self.try_downcast_data() {
                    return a.infix(op, other, env);
                }
                if let Some(b) = other.try_downcast_data() {
                    return self.infix(op, &b, env);
                }
                match op {
                    "+" => {
                        try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                            return Ok(Value::Data(Data::new_dynamic(a + b)));
                        } else {
                            #[cfg(feature = "glam")]
                            try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                return Ok(Value::Data(Data::new_dynamic(*a + *b)));
                            } else {
                                return Err(anyhow::anyhow!("Invalid data types for operator `+`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                            });
                            #[cfg(not(feature = "glam"))]
                            Err(anyhow::anyhow!("Invalid data types for operator `+`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                        })
                    }
                    "-" => {
                        try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                            return Ok(Value::Data(Data::new_dynamic(a - b)));
                        } else {
                            #[cfg(feature = "glam")]
                            try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                return Ok(Value::Data(Data::new_dynamic(*a - *b)));
                            } else {
                                return Err(anyhow::anyhow!("Invalid data types for operator `-`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                            });
                            #[cfg(not(feature = "glam"))]
                            Err(anyhow::anyhow!("Invalid data types for operator `-`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                        })
                    }
                    "*" => {
                        try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                            return Ok(Value::Data(Data::new_dynamic(a * b)));
                        } else {
                            #[cfg(feature = "glam")]
                            try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                return Ok(Value::Data(Data::new_dynamic(*a * *b)));
                            } else {
                                return Err(anyhow::anyhow!("Invalid data types for operator `*`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                            });
                            #[cfg(not(feature = "glam"))]
                            Err(anyhow::anyhow!("Invalid data types for operator `*`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                        })
                    }
                    "/" => {
                        try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                            return Ok(Value::Data(Data::new_dynamic(a / b)));
                        } else {
                            #[cfg(feature = "glam")]
                            try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4; |a, b| {
                                return Ok(Value::Data(Data::new_dynamic(*a / *b)));
                            } else {
                                return Err(anyhow::anyhow!("Invalid data types for operator `/`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                            });
                            #[cfg(not(feature = "glam"))]
                            Err(anyhow::anyhow!("Invalid data types for operator `/`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                        })
                    }
                    "%" => {
                        try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                            return Ok(Value::Data(Data::new_dynamic(a % b)));
                        } else {
                            Err(anyhow::anyhow!("Invalid data types for operator `%`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                        })
                    }
                    _ => bail!("Invalid infix operator for data: {}", op),
                }
            }
            _ => bail!("Invalid infix operator for value: {}", op),
        }
    }

    #[allow(clippy::needless_return)]
    pub fn infix_mut(&mut self, op: &str, other: &Value, env: &RuntimeEnv) -> Result<Value> {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => match op {
                "+" => Ok(Value::Int(*a + b)),
                "-" => Ok(Value::Int(*a - b)),
                "*" => Ok(Value::Int(*a * b)),
                "/" => Ok(Value::Int(*a / b)),
                "%" => Ok(Value::Int(*a % b)),
                "=" => {
                    *a = *b;
                    Ok(Value::Int(*a))
                }
                "+=" => {
                    *a += b;
                    Ok(Value::Int(*a))
                }
                "-=" => {
                    *a -= b;
                    Ok(Value::Int(*a))
                }
                "*=" => {
                    *a *= b;
                    Ok(Value::Int(*a))
                }
                "/=" => {
                    *a /= b;
                    Ok(Value::Int(*a))
                }
                "%=" => {
                    *a %= b;
                    Ok(Value::Int(*a))
                }
                _ => bail!("Invalid infix operator for integer: {}", op),
            },
            (Value::Float(a), Value::Float(b)) => match op {
                "+" => Ok(Value::Float(*a + b)),
                "-" => Ok(Value::Float(*a - b)),
                "*" => Ok(Value::Float(*a * b)),
                "/" => Ok(Value::Float(*a / b)),
                "%" => Ok(Value::Float(*a % b)),
                "=" => {
                    *a = *b;
                    Ok(Value::Float(*a))
                }
                "+=" => {
                    *a += b;
                    Ok(Value::Float(*a))
                }
                "-=" => {
                    *a -= b;
                    Ok(Value::Float(*a))
                }
                "*=" => {
                    *a *= b;
                    Ok(Value::Float(*a))
                }
                "/=" => {
                    *a /= b;
                    Ok(Value::Float(*a))
                }
                "%=" => {
                    *a %= b;
                    Ok(Value::Float(*a))
                }
                _ => bail!("Invalid infix operator for float: {}", op),
            },
            (a, Value::Int(b)) => {
                let b = Value::Data(Data::new_dynamic(*b));
                a.infix_mut(op, &b, env)
            }
            (Value::Int(a), Value::Data(_)) => {
                let mut a = Value::Data(Data::new_dynamic(*a));
                a.infix_mut(op, other, env)
            }
            (a, Value::Float(b)) => {
                let b = Value::Data(Data::new_dynamic(*b));
                a.infix_mut(op, &b, env)
            }
            (Value::Float(a), Value::Data(_)) => {
                let mut a = Value::Data(Data::new_dynamic(*a));
                a.infix_mut(op, other, env)
            }
            (a, b) => {
                if let Some(b) = b.as_data().and_then(|b| b.as_pointer()) {
                    // deref
                    let b = {
                        let world = env.world.read();
                        // let b = world
                        //     .storage()
                        //     .find(b.target_type_uid(), b.target_value_uid())?;
                        let b = world
                            .get(b.target_value_uid(), b.target_type_uid())
                            .unwrap();
                        let b = b.to_owned().unwrap();
                        Value::Data(b)
                    };
                    return a.infix_mut(op, &b, env);
                }
                if let Some(b) = other.try_downcast_data() {
                    return a.infix_mut(op, &b, env);
                }
                if let Some(a) = a.as_data().and_then(|a| a.as_pointer()) {
                    // deref mut
                    let world = env.world.read();
                    let mut a = world
                        .get_mut(a.target_value_uid(), a.target_type_uid())
                        .unwrap();

                    if let Value::Data(b) = b {
                        match op {
                            "+" => {
                                try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                    return Ok(Value::Data(Data::new_dynamic(*a + *b)));
                                } else {
                                    #[cfg(feature = "glam")]
                                    try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                        return Ok(Value::Data(Data::new_dynamic(*a + *b)));
                                    } else {
                                        return Err(anyhow::anyhow!("Invalid data types for operator `+`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                                    });
                                    #[cfg(not(feature = "glam"))]
                                    Err(anyhow::anyhow!("Invalid data types for operator `+`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                                })
                            }
                            "-" => {
                                try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                    return Ok(Value::Data(Data::new_dynamic(*a - *b)));
                                } else {
                                    #[cfg(feature = "glam")]
                                    try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                        return Ok(Value::Data(Data::new_dynamic(*a - *b)));
                                    } else {
                                        return Err(anyhow::anyhow!("Invalid data types for operator `-`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                                    });
                                    #[cfg(not(feature = "glam"))]
                                    Err(anyhow::anyhow!("Invalid data types for operator `-`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                                })
                            }
                            "*" => {
                                try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                    return Ok(Value::Data(Data::new_dynamic(*a * *b)));
                                } else {
                                    #[cfg(feature = "glam")]
                                    try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                        return Ok(Value::Data(Data::new_dynamic(*a * *b)));
                                    } else {
                                        return Err(anyhow::anyhow!("Invalid data types for operator `*`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                                    });
                                    #[cfg(not(feature = "glam"))]
                                    Err(anyhow::anyhow!("Invalid data types for operator `*`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                                })
                            }
                            "/" => {
                                try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                    return Ok(Value::Data(Data::new_dynamic(*a / *b)));
                                } else {
                                    #[cfg(feature = "glam")]
                                    try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4; |a, b| {
                                        return Ok(Value::Data(Data::new_dynamic(*a / *b)));
                                    } else {
                                        return Err(anyhow::anyhow!("Invalid data types for operator `/`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                                    });
                                    #[cfg(not(feature = "glam"))]
                                    Err(anyhow::anyhow!("Invalid data types for operator `/`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                                })
                            }
                            "%" => {
                                try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                    return Ok(Value::Data(Data::new_dynamic(*a % *b)));
                                } else {
                                    Err(anyhow::anyhow!("Invalid data types for operator `%`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                                })
                            }
                            "=" => {
                                try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                    *a = *b;
                                    return Ok(Value::Void);
                                } else {
                                    #[cfg(feature = "glam")]
                                    try_all_types_both_mut_lhs!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                        *a = *b;
                                        return Ok(Value::Void);
                                    } else {
                                        return Err(anyhow::anyhow!("Invalid data types for operator `=`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                                    });
                                    #[cfg(not(feature = "glam"))]
                                    Err(anyhow::anyhow!("Invalid data types for operator `=`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                                })
                            }
                            "+=" => {
                                try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                    *a += b;
                                    return Ok(Value::Void);
                                } else {
                                    #[cfg(feature = "glam")]
                                    try_all_types_both_mut_lhs!(a, b; glam::Vec2, glam::Vec3, glam::Vec4; |a, b| {
                                        *a += *b;
                                        return Ok(Value::Void);
                                    } else {
                                        return Err(anyhow::anyhow!("Invalid data types for operator `+=`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                                    });
                                    #[cfg(not(feature = "glam"))]
                                    Err(anyhow::anyhow!("Invalid data types for operator `+=`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                                })
                            }
                            "-=" => {
                                try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                    *a -= b;
                                    return Ok(Value::Void);
                                } else {
                                    #[cfg(feature = "glam")]
                                    try_all_types_both_mut_lhs!(a, b; glam::Vec2, glam::Vec3, glam::Vec4; |a, b| {
                                        *a -= *b;
                                        return Ok(Value::Void);
                                    } else {
                                        return Err(anyhow::anyhow!("Invalid data types for operator `-=`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                                    });
                                    #[cfg(not(feature = "glam"))]
                                    Err(anyhow::anyhow!("Invalid data types for operator `-=`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                                })
                            }
                            "*=" => {
                                try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                    *a *= b;
                                    return Ok(Value::Void);
                                } else {
                                    #[cfg(feature = "glam")]
                                    try_all_types_both_mut_lhs!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                        *a *= *b;
                                        return Ok(Value::Void);
                                    } else {
                                        return Err(anyhow::anyhow!("Invalid data types for operator `*=`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                                    });
                                    #[cfg(not(feature = "glam"))]
                                    Err(anyhow::anyhow!("Invalid data types for operator `*=`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                                })
                            }
                            "/=" => {
                                try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                    *a /= b;
                                    return Ok(Value::Void);
                                } else {
                                    #[cfg(feature = "glam")]
                                    try_all_types_both_mut_lhs!(a, b; glam::Vec2, glam::Vec3, glam::Vec4; |a, b| {
                                        *a /= *b;
                                        return Ok(Value::Void);
                                    } else {
                                        return Err(anyhow::anyhow!("Invalid data types for operator `/=`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                                    });
                                    #[cfg(not(feature = "glam"))]
                                    Err(anyhow::anyhow!("Invalid data types for operator `/=`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                                })
                            }
                            "%=" => {
                                try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                    *a %= b;
                                    return Ok(Value::Void);
                                } else {
                                    Err(anyhow::anyhow!("Invalid data types for operator `%=`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                                })
                            }
                            _ => todo!(),
                        }
                    } else {
                        todo!()
                    }
                } else if let (Value::Data(a), Value::Data(b)) = (a, b) {
                    match op {
                        "+" => {
                            try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                return Ok(Value::Data(Data::new_dynamic(a + b)));
                            } else {
                                #[cfg(feature = "glam")]
                                try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                    return Ok(Value::Data(Data::new_dynamic(*a + *b)))
                                } else {
                                    return Err(anyhow::anyhow!("Invalid data types for operator `+`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                                });
                                #[cfg(not(feature = "glam"))]
                                Err(anyhow::anyhow!("Invalid data types for operator `+`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                            })
                        }
                        "-" => {
                            try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                return Ok(Value::Data(Data::new_dynamic(a - b)));
                            } else {
                                #[cfg(feature = "glam")]
                                try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                    return Ok(Value::Data(Data::new_dynamic(*a - *b)))
                                } else {
                                    return Err(anyhow::anyhow!("Invalid data types for operator `-`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                                });
                                #[cfg(not(feature = "glam"))]
                                Err(anyhow::anyhow!("Invalid data types for operator `-`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                            })
                        }
                        "*" => {
                            try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                return Ok(Value::Data(Data::new_dynamic(a * b)));
                            } else {
                                #[cfg(feature = "glam")]
                                try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                    return Ok(Value::Data(Data::new_dynamic(*a * *b)))
                                } else {
                                    return Err(anyhow::anyhow!("Invalid data types for operator `*`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                                });
                                #[cfg(not(feature = "glam"))]
                                Err(anyhow::anyhow!("Invalid data types for operator `*`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                            })
                        }
                        "/" => {
                            try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                return Ok(Value::Data(Data::new_dynamic(a / b)));
                            } else {
                                #[cfg(feature = "glam")]
                                try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4; |a, b| {
                                    return Ok(Value::Data(Data::new_dynamic(*a / *b)))
                                } else {
                                    return Err(anyhow::anyhow!("Invalid data types for operator `/`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                                });
                                #[cfg(not(feature = "glam"))]
                                Err(anyhow::anyhow!("Invalid data types for operator `/`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                            })
                        }
                        "%" => {
                            try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                return Ok(Value::Data(Data::new_dynamic(a % b)));
                            } else {
                                return Err(anyhow::anyhow!("Invalid data types for operator `%`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                            })
                        }
                        "=" => {
                            try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                *a = *b;
                                return Ok(Value::Data(Data::new_dynamic(*a)));
                            } else {
                                #[cfg(feature = "glam")]
                                try_all_types_both_mut_lhs!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                    *a = *b;
                                    return Ok(Value::Data(Data::new_dynamic(*a)));
                                } else {
                                    return Err(anyhow::anyhow!("Invalid data types for operator `=`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                                });
                                #[cfg(not(feature = "glam"))]
                                Err(anyhow::anyhow!("Invalid data types for operator `=`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                            })
                        }
                        "+=" => {
                            try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                *a += b;
                                return Ok(Value::Data(Data::new_dynamic(*a)));
                            } else {
                                #[cfg(feature = "glam")]
                                try_all_types_both_mut_lhs!(a, b; glam::Vec2, glam::Vec3, glam::Vec4; |a, b| {
                                    *a += *b;
                                    return Ok(Value::Data(Data::new_dynamic(*a)));
                                } else {
                                    return Err(anyhow::anyhow!("Invalid data types for operator `+=`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                                });
                                #[cfg(not(feature = "glam"))]
                                Err(anyhow::anyhow!("Invalid data types for operator `+=`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                            })
                        }
                        "-=" => {
                            try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                *a -= b;
                                return Ok(Value::Data(Data::new_dynamic(*a)));
                            } else {
                                #[cfg(feature = "glam")]
                                try_all_types_both_mut_lhs!(a, b; glam::Vec2, glam::Vec3, glam::Vec4; |a, b| {
                                    *a -= *b;
                                    return Ok(Value::Data(Data::new_dynamic(*a)));
                                } else {
                                    return Err(anyhow::anyhow!("Invalid data types for operator `-=`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                                });
                                #[cfg(not(feature = "glam"))]
                                Err(anyhow::anyhow!("Invalid data types for operator `-=`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                            })
                        }
                        "*=" => {
                            try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                *a *= b;
                                return Ok(Value::Data(Data::new_dynamic(*a)));
                            } else {
                                #[cfg(feature = "glam")]
                                try_all_types_both_mut_lhs!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                    *a *= *b;
                                    return Ok(Value::Data(Data::new_dynamic(*a)));
                                } else {
                                    return Err(anyhow::anyhow!("Invalid data types for operator `*=`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                                });
                                #[cfg(not(feature = "glam"))]
                                Err(anyhow::anyhow!("Invalid data types for operator `*=`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                            })
                        }
                        "/=" => {
                            try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                *a /= b;
                                return Ok(Value::Data(Data::new_dynamic(*a)));
                            } else {
                                #[cfg(feature = "glam")]
                                try_all_types_both_mut_lhs!(a, b; glam::Vec2, glam::Vec3, glam::Vec4; |a, b| {
                                    *a /= *b;
                                    return Ok(Value::Data(Data::new_dynamic(*a)));
                                } else {
                                    return Err(anyhow::anyhow!("Invalid data types for operator `/=`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                                });
                                #[cfg(not(feature = "glam"))]
                                Err(anyhow::anyhow!("Invalid data types for operator `/=`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()))
                            })
                        }
                        "%=" => {
                            try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                *a %= b;
                                return Ok(Value::Data(Data::new_dynamic(*a)));
                            } else {
                                return Err(anyhow::anyhow!("Invalid data types for operator `%=`: {:?} and {:?}", a.type_uid().type_name(), b.type_uid().type_name()));
                            })
                        }
                        _ => bail!("Invalid infix operator for data: {}", op),
                    }
                } else {
                    bail!("Invalid infix operator for value: {}", op)
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ValueHandle {
    Ref(SharedLock<Value>),
    Mut(SharedLock<Value>),
}

impl ValueHandle {
    pub fn new(value: Value) -> Self {
        Self::Mut(SharedLock::new(value))
    }

    pub fn read(&self) -> Read<'_, Value> {
        match self {
            Self::Ref(lock) => lock.read(),
            Self::Mut(lock) => lock.read(),
        }
    }

    pub fn deep_copy(&self) -> Value {
        match self {
            Self::Ref(lock) => lock.read().to_owned(),
            Self::Mut(lock) => lock.read().to_owned(),
        }
    }

    pub fn display(&self, env: &RuntimeEnv) -> String {
        self.read().display(env)
    }

    pub fn infix(&self, op: &str, other: &ValueHandle, env: &RuntimeEnv) -> Result<Value> {
        match (self, other) {
            (Self::Ref(a), Self::Ref(b)) => a.read().infix(op, &b.read(), env),
            (Self::Ref(a), Self::Mut(b)) => a.read().infix(op, &b.read(), env),
            (Self::Mut(a), Self::Ref(b)) => a.write().infix_mut(op, &b.read(), env),
            (Self::Mut(a), Self::Mut(b)) => a.write().infix_mut(op, &b.read(), env),
        }
    }

    pub fn prefix(&self, op: &str) -> Result<Value> {
        match self {
            Self::Ref(a) => a.read().prefix(op),
            Self::Mut(a) => a.write().prefix(op),
        }
    }
}

impl From<Value> for ValueHandle {
    fn from(value: Value) -> Self {
        Self::Mut(SharedLock::new(value))
    }
}
