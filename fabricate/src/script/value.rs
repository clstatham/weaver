use anyhow::{bail, Result};

use crate::{
    lock::Write,
    prelude::{Data, Entity, Read, SharedLock},
    query::QueryBuilderAccess, world::LockedWorldHandle,
};

use super::interp::InterpreterContext;

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

#[derive(Debug, PartialEq)]
pub(super) enum Value {
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
    pub fn display(&self) -> String {
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

    pub fn as_data(&self) -> Option<&Data> {
        match self {
            Value::Data(d) => Some(d),
            _ => None,
        }
    }

    pub fn to_owned_data(&self, world: &LockedWorldHandle) -> Result<Data> {
        match self {
            Value::Data(d) => Ok(d.to_owned()),
            Value::Bool(b) => Ok(Data::new_dynamic(world, *b)),
            Value::Int(i) => Ok(Data::new_dynamic(world, *i)),
            Value::Float(f) => Ok(Data::new_dynamic(world, *f)),
            Value::String(s) => Ok(Data::new_dynamic(world, s.clone())),
            Value::Resource(_) => bail!("Cannot convert Resource to Data"),
            Value::Query(_) => bail!("Cannot convert Query to Data"),
            Value::Void => bail!("Cannot convert Void to Data"),
        }
    }

    pub fn despawn(&self, world: &LockedWorldHandle) {
        if let Value::Data(d) = self {
            if d.entity().is_alive() {
                d.entity().kill(world);
            }
        }
    }

    pub fn try_downcast_data(&self, world: &LockedWorldHandle) -> Option<Self> {
        if let Value::Data(d) = self {
            if let Some(b) = d.as_ref::<bool>() {
                self.despawn(world);
                return Some(Value::Bool(*b));
            }
            try_all_types!(d; i64, u64, i32, u32, i16, u16, i8, u8; |data| {
                self.despawn(world);
                #[allow(clippy::unnecessary_cast)]
                return Some(Value::Int(*data as i64));
            } else {});
            try_all_types!(d; f32, f64; |data| {
                self.despawn(world);
                #[allow(clippy::unnecessary_cast)]
                return Some(Value::Float(*data as f32));
            } else {});
            try_all_types!(d; String; |data| {
                self.despawn(world);
                return Some(Value::String(data.clone()));
            } else {});
            
            return None
        }
        None
    }

    #[allow(clippy::needless_return)]
    fn infix(&self, op: &str, other: &Value, ctx: &mut InterpreterContext) -> Result<ValueHandle> {
        match (self, other) {
            (Value::Bool(a), Value::Bool(b)) => match op {
                "&&" => Ok(ctx.alloc_value(Value::Bool(*a && *b))),
                "||" => Ok(ctx.alloc_value(Value::Bool(*a || *b))),
                "^" => Ok(ctx.alloc_value(Value::Bool(*a ^ *b))),
                _ => bail!("Invalid infix operator for boolean: {}", op),
            },
            (Value::Int(a), Value::Int(b)) => match op {
                "+" => Ok(ctx.alloc_value(Value::Int(a + b))),
                "-" => Ok(ctx.alloc_value(Value::Int(a - b))),
                "*" => Ok(ctx.alloc_value(Value::Int(a * b))),
                "/" => Ok(ctx.alloc_value(Value::Int(a / b))),
                "%" => Ok(ctx.alloc_value(Value::Int(a % b))),
                _ => bail!("Invalid infix operator for integer: {}", op),
            },
            (Value::Float(a), Value::Float(b)) => match op {
                "+" => Ok(ctx.alloc_value(Value::Float(a + b))),
                "-" => Ok(ctx.alloc_value(Value::Float(a - b))),
                "*" => Ok(ctx.alloc_value(Value::Float(a * b))),
                "/" => Ok(ctx.alloc_value(Value::Float(a / b))),
                "%" => Ok(ctx.alloc_value(Value::Float(a % b))),
                _ => bail!("Invalid infix operator for float: {}", op),
            },
            (Value::Data(_), Value::Int(b)) => {
                let b = ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *b)));
                let b = b.read();
                self.infix(op, &b, ctx)
            }
            (Value::Int(a), Value::Data(_)) => {
                let a = ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a)));
                let a = a.read();
                a.infix(op, other, ctx)
            }
            (Value::Data(_), Value::Float(b)) => {
                let b = ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *b)));
                let b = b.read();
                self.infix(op, &b, ctx)
            }
            (Value::Float(a), Value::Data(_)) => {
                let a = ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a)));
                let a = a.read();
                a.infix(op, other, ctx)
            }
            (Value::Data(a), Value::Data(b)) => {
                if let Some(b) = b.as_pointer() {
                    // deref
                    return b
                        .with_deref(&ctx.world.clone(), |b| {
                            let b = b.to_owned().unwrap();
                            let b = Value::Data(b);
                            return self.infix(op, &b, ctx);
                        })
                        .unwrap();
                }
                if let Some(a) = a.as_pointer() {
                    // deref
                    return a
                        .with_deref(&ctx.world.clone(), |a| {
                            let a = a.to_owned().unwrap();
                            let a = Value::Data(a);
                            return a.infix(op, other, ctx);
                        })
                        .unwrap();
                }
                if let Some(a) = self.try_downcast_data(&ctx.world) {
                    return a.infix(op, other, ctx);
                }
                if let Some(b) = other.try_downcast_data(&ctx.world) {
                    return self.infix(op, &b, ctx);
                }
                match op {
                    "+" => {
                        try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                            return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, a + b))));
                        } else {
                            #[cfg(feature = "glam")]
                            try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a + *b))));
                            } else {
                                return Err(anyhow::anyhow!("Invalid data types for operator `+`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                            });
                            #[cfg(not(feature = "glam"))]
                            Err(anyhow::anyhow!("Invalid data types for operator `+`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                        })
                    }
                    "-" => {
                        try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                            return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, a - b))));
                        } else {
                            #[cfg(feature = "glam")]
                            try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a - *b))));
                            } else {
                                return Err(anyhow::anyhow!("Invalid data types for operator `-`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                            });
                            #[cfg(not(feature = "glam"))]
                            Err(anyhow::anyhow!("Invalid data types for operator `-`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                        })
                    }
                    "*" => {
                        try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                            return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, a * b))));
                        } else {
                            #[cfg(feature = "glam")]
                            try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a * *b))));
                            } else {
                                return Err(anyhow::anyhow!("Invalid data types for operator `*`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                            });
                            #[cfg(not(feature = "glam"))]
                            Err(anyhow::anyhow!("Invalid data types for operator `*`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                        })
                    }
                    "/" => {
                        try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                            return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, a / b))));
                        } else {
                            #[cfg(feature = "glam")]
                            try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4; |a, b| {
                                return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a / *b))));
                            } else {
                                return Err(anyhow::anyhow!("Invalid data types for operator `/`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                            });
                            #[cfg(not(feature = "glam"))]
                            Err(anyhow::anyhow!("Invalid data types for operator `/`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                        })
                    }
                    "%" => {
                        try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                            return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, a % b))));
                        } else {
                            Err(anyhow::anyhow!("Invalid data types for operator `%`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                        })
                    }
                    _ => bail!("Invalid infix operator for data: {}", op),
                }
            }
            _ => bail!("Invalid infix operator for value: {}", op),
        }
    }

    #[allow(clippy::needless_return)]
    pub fn infix_mut(
        &mut self,
        op: &str,
        other: &Value,
        ctx: &mut InterpreterContext,
    ) -> Result<ValueHandle> {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => match op {
                "+" => Ok(ctx.alloc_value(Value::Int(*a + b))),
                "-" => Ok(ctx.alloc_value(Value::Int(*a - b))),
                "*" => Ok(ctx.alloc_value(Value::Int(*a * b))),
                "/" => Ok(ctx.alloc_value(Value::Int(*a / b))),
                "%" => Ok(ctx.alloc_value(Value::Int(*a % b))),
                "=" => {
                    *a = *b;
                    Ok(ctx.alloc_value(Value::Int(*a)))
                }
                "+=" => {
                    *a += b;
                    Ok(ctx.alloc_value(Value::Int(*a)))
                }
                "-=" => {
                    *a -= b;
                    Ok(ctx.alloc_value(Value::Int(*a)))
                }
                "*=" => {
                    *a *= b;
                    Ok(ctx.alloc_value(Value::Int(*a)))
                }
                "/=" => {
                    *a /= b;
                    Ok(ctx.alloc_value(Value::Int(*a)))
                }
                "%=" => {
                    *a %= b;
                    Ok(ctx.alloc_value(Value::Int(*a)))
                }
                _ => bail!("Invalid infix operator for integer: {}", op),
            },
            (Value::Float(a), Value::Float(b)) => match op {
                "+" => Ok(ctx.alloc_value(Value::Float(*a + b))),
                "-" => Ok(ctx.alloc_value(Value::Float(*a - b))),
                "*" => Ok(ctx.alloc_value(Value::Float(*a * b))),
                "/" => Ok(ctx.alloc_value(Value::Float(*a / b))),
                "%" => Ok(ctx.alloc_value(Value::Float(*a % b))),
                "=" => {
                    *a = *b;
                    Ok(ctx.alloc_value(Value::Float(*a)))
                }
                "+=" => {
                    *a += b;
                    Ok(ctx.alloc_value(Value::Float(*a)))
                }
                "-=" => {
                    *a -= b;
                    Ok(ctx.alloc_value(Value::Float(*a)))
                }
                "*=" => {
                    *a *= b;
                    Ok(ctx.alloc_value(Value::Float(*a)))
                }
                "/=" => {
                    *a /= b;
                    Ok(ctx.alloc_value(Value::Float(*a)))
                }
                "%=" => {
                    *a %= b;
                    Ok(ctx.alloc_value(Value::Float(*a)))
                }
                _ => bail!("Invalid infix operator for float: {}", op),
            },
            (a, Value::Int(b)) => {
                let b = ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *b)));
                let b = b.read();
                a.infix_mut(op, &b, ctx)
            }
            (Value::Int(a), Value::Data(_)) => {
                let a = ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a)));
                let mut a = a.write();
                a.infix_mut(op, other, ctx)
            }
            (a, Value::Float(b)) => {
                let b = ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *b)));
                let b = b.read();
                a.infix_mut(op, &b, ctx)
            }
            (Value::Float(a), Value::Data(_)) => {
                let a = ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a)));
                let mut a = a.write();
                a.infix_mut(op, other, ctx)
            }
            (a, b) => {
                if let Some(b) = b.as_data().and_then(|b| b.as_pointer()) {
                    // deref
                    return b
                        .with_deref_mut(&ctx.world.clone(), |b| {
                            let b = b.to_owned();
                            let b = Value::Data(b);
                            return a.infix_mut(op, &b, ctx);
                        })
                        .unwrap();
                }
                if let Some(b) = other.try_downcast_data(&ctx.world) {
                    return a.infix_mut(op, &b, ctx);
                }
                if let Some(a) = a.as_data().and_then(|a| a.as_pointer()) {
                    // deref mut
                    a.with_deref_mut(&ctx.world.clone(), |mut a| {
                        
                    if let Value::Data(b) = b {
                        match op {
                            "+" => {
                                try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                    return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a + *b))));
                                } else {
                                    #[cfg(feature = "glam")]
                                    try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                        return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a + *b))));
                                    } else {
                                        return Err(anyhow::anyhow!("Invalid data types for operator `+`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                                    });
                                    #[cfg(not(feature = "glam"))]
                                    Err(anyhow::anyhow!("Invalid data types for operator `+`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                                })
                            }
                            "-" => {
                                try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                    return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a - *b))));
                                } else {
                                    #[cfg(feature = "glam")]
                                    try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                        return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a - *b))));
                                    } else {
                                        return Err(anyhow::anyhow!("Invalid data types for operator `-`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                                    });
                                    #[cfg(not(feature = "glam"))]
                                    Err(anyhow::anyhow!("Invalid data types for operator `-`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                                })
                            }
                            "*" => {
                                try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                    return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a * *b))));
                                } else {
                                    #[cfg(feature = "glam")]
                                    try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                        return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a * *b))));
                                    } else {
                                        return Err(anyhow::anyhow!("Invalid data types for operator `*`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                                    });
                                    #[cfg(not(feature = "glam"))]
                                    Err(anyhow::anyhow!("Invalid data types for operator `*`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                                })
                            }
                            "/" => {
                                try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                    return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a / *b))));
                                } else {
                                    #[cfg(feature = "glam")]
                                    try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4; |a, b| {
                                        return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a / *b))));
                                    } else {
                                        return Err(anyhow::anyhow!("Invalid data types for operator `/`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                                    });
                                    #[cfg(not(feature = "glam"))]
                                    Err(anyhow::anyhow!("Invalid data types for operator `/`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                                })
                            }
                            "%" => {
                                try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                    return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a % *b))));
                                } else {
                                    Err(anyhow::anyhow!("Invalid data types for operator `%`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                                })
                            }
                            "=" => {
                                try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                    *a = *b;
                                    return Ok(ctx.alloc_value(Value::Void));
                                } else {
                                    #[cfg(feature = "glam")]
                                    try_all_types_both_mut_lhs!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                        *a = *b;
                                        return Ok(ctx.alloc_value(Value::Void));
                                    } else {
                                        return Err(anyhow::anyhow!("Invalid data types for operator `=`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                                    });
                                    #[cfg(not(feature = "glam"))]
                                    Err(anyhow::anyhow!("Invalid data types for operator `=`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                                })
                            }
                            "+=" => {
                                try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                    *a += b;
                                    return Ok(ctx.alloc_value(Value::Void));
                                } else {
                                    #[cfg(feature = "glam")]
                                    try_all_types_both_mut_lhs!(a, b; glam::Vec2, glam::Vec3, glam::Vec4; |a, b| {
                                        *a += *b;
                                        return Ok(ctx.alloc_value(Value::Void));
                                    } else {
                                        return Err(anyhow::anyhow!("Invalid data types for operator `+=`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                                    });
                                    #[cfg(not(feature = "glam"))]
                                    Err(anyhow::anyhow!("Invalid data types for operator `+=`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                                })
                            }
                            "-=" => {
                                try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                    *a -= b;
                                    return Ok(ctx.alloc_value(Value::Void));
                                } else {
                                    #[cfg(feature = "glam")]
                                    try_all_types_both_mut_lhs!(a, b; glam::Vec2, glam::Vec3, glam::Vec4; |a, b| {
                                        *a -= *b;
                                        return Ok(ctx.alloc_value(Value::Void));
                                    } else {
                                        return Err(anyhow::anyhow!("Invalid data types for operator `-=`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                                    });
                                    #[cfg(not(feature = "glam"))]
                                    Err(anyhow::anyhow!("Invalid data types for operator `-=`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                                })
                            }
                            "*=" => {
                                try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                    *a *= b;
                                    return Ok(ctx.alloc_value(Value::Void));
                                } else {
                                    #[cfg(feature = "glam")]
                                    try_all_types_both_mut_lhs!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                        *a *= *b;
                                        return Ok(ctx.alloc_value(Value::Void));
                                    } else {
                                        return Err(anyhow::anyhow!("Invalid data types for operator `*=`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                                    });
                                    #[cfg(not(feature = "glam"))]
                                    Err(anyhow::anyhow!("Invalid data types for operator `*=`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                                })
                            }
                            "/=" => {
                                try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                    *a /= b;
                                    return Ok(ctx.alloc_value(Value::Void));
                                } else {
                                    #[cfg(feature = "glam")]
                                    try_all_types_both_mut_lhs!(a, b; glam::Vec2, glam::Vec3, glam::Vec4; |a, b| {
                                        *a /= *b;
                                        return Ok(ctx.alloc_value(Value::Void));
                                    } else {
                                        return Err(anyhow::anyhow!("Invalid data types for operator `/=`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                                    });
                                    #[cfg(not(feature = "glam"))]
                                    Err(anyhow::anyhow!("Invalid data types for operator `/=`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                                })
                            }
                            "%=" => {
                                try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                    *a %= b;
                                    return Ok(ctx.alloc_value(Value::Void));
                                } else {
                                    Err(anyhow::anyhow!("Invalid data types for operator `%=`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                                })
                            }
                            _ => todo!(),
                        }
                    } else {
                        todo!()
                    }
                    })?
                } else if let (Value::Data(a), Value::Data(b)) = (a, b) {
                    match op {
                        "+" => {
                            try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, a + b))));
                            } else {
                                #[cfg(feature = "glam")]
                                try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                    return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a + *b))))
                                } else {
                                    return Err(anyhow::anyhow!("Invalid data types for operator `+`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                                });
                                #[cfg(not(feature = "glam"))]
                                Err(anyhow::anyhow!("Invalid data types for operator `+`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                            })
                        }
                        "-" => {
                            try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, a - b))));
                            } else {
                                #[cfg(feature = "glam")]
                                try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                    return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a - *b))))
                                } else {
                                    return Err(anyhow::anyhow!("Invalid data types for operator `-`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                                });
                                #[cfg(not(feature = "glam"))]
                                Err(anyhow::anyhow!("Invalid data types for operator `-`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                            })
                        }
                        "*" => {
                            try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, a * b))));
                            } else {
                                #[cfg(feature = "glam")]
                                try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                    return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a * *b))))
                                } else {
                                    return Err(anyhow::anyhow!("Invalid data types for operator `*`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                                });
                                #[cfg(not(feature = "glam"))]
                                Err(anyhow::anyhow!("Invalid data types for operator `*`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                            })
                        }
                        "/" => {
                            try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, a / b))));
                            } else {
                                #[cfg(feature = "glam")]
                                try_all_types_both!(a, b; glam::Vec2, glam::Vec3, glam::Vec4; |a, b| {
                                    return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a / *b))))
                                } else {
                                    return Err(anyhow::anyhow!("Invalid data types for operator `/`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                                });
                                #[cfg(not(feature = "glam"))]
                                Err(anyhow::anyhow!("Invalid data types for operator `/`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                            })
                        }
                        "%" => {
                            try_all_types_both!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, a % b))));
                            } else {
                                return Err(anyhow::anyhow!("Invalid data types for operator `%`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                            })
                        }
                        "=" => {
                            try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                *a = *b;
                                return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a))));
                            } else {
                                #[cfg(feature = "glam")]
                                try_all_types_both_mut_lhs!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                    *a = *b;
                                    return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a))));
                                } else {
                                    return Err(anyhow::anyhow!("Invalid data types for operator `=`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                                });
                                #[cfg(not(feature = "glam"))]
                                Err(anyhow::anyhow!("Invalid data types for operator `=`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                            })
                        }
                        "+=" => {
                            try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                *a += b;
                                return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a))));
                            } else {
                                #[cfg(feature = "glam")]
                                try_all_types_both_mut_lhs!(a, b; glam::Vec2, glam::Vec3, glam::Vec4; |a, b| {
                                    *a += *b;
                                    return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a))));
                                } else {
                                    return Err(anyhow::anyhow!("Invalid data types for operator `+=`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                                });
                                #[cfg(not(feature = "glam"))]
                                Err(anyhow::anyhow!("Invalid data types for operator `+=`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                            })
                        }
                        "-=" => {
                            try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                *a -= b;
                                return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a))));
                            } else {
                                #[cfg(feature = "glam")]
                                try_all_types_both_mut_lhs!(a, b; glam::Vec2, glam::Vec3, glam::Vec4; |a, b| {
                                    *a -= *b;
                                    return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a))));
                                } else {
                                    return Err(anyhow::anyhow!("Invalid data types for operator `-=`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                                });
                                #[cfg(not(feature = "glam"))]
                                Err(anyhow::anyhow!("Invalid data types for operator `-=`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                            })
                        }
                        "*=" => {
                            try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                *a *= b;
                                return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a))));
                            } else {
                                #[cfg(feature = "glam")]
                                try_all_types_both_mut_lhs!(a, b; glam::Vec2, glam::Vec3, glam::Vec4, glam::Quat; |a, b| {
                                    *a *= *b;
                                    return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a))));
                                } else {
                                    return Err(anyhow::anyhow!("Invalid data types for operator `*=`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                                });
                                #[cfg(not(feature = "glam"))]
                                Err(anyhow::anyhow!("Invalid data types for operator `*=`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                            })
                        }
                        "/=" => {
                            try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                *a /= b;
                                return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a))));
                            } else {
                                #[cfg(feature = "glam")]
                                try_all_types_both_mut_lhs!(a, b; glam::Vec2, glam::Vec3, glam::Vec4; |a, b| {
                                    *a /= *b;
                                    return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a))));
                                } else {
                                    return Err(anyhow::anyhow!("Invalid data types for operator `/=`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
                                });
                                #[cfg(not(feature = "glam"))]
                                Err(anyhow::anyhow!("Invalid data types for operator `/=`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()))
                            })
                        }
                        "%=" => {
                            try_all_types_both_mut_lhs!(a, b; u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64; |a, b| {
                                *a %= b;
                                return Ok(ctx.alloc_value(Value::Data(Data::new_dynamic(&ctx.world, *a))));
                            } else {
                                return Err(anyhow::anyhow!("Invalid data types for operator `%=`: {:?} and {:?}", a.type_id().type_name(), b.type_id().type_name()));
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
pub(super) enum ValueHandle {
    Ref(SharedLock<Value>),
    Mut(SharedLock<Value>),
}

impl ValueHandle {
    pub fn read(&self) -> Read<'_, Value> {
        match self {
            Self::Ref(lock) => lock.read(),
            Self::Mut(lock) => lock.read(),
        }
    }

    pub fn write(&self) -> Write<'_, Value> {
        match self {
            Self::Ref(lock) => lock.write(),
            Self::Mut(lock) => lock.write(),
        }
    }

    pub fn display(&self) -> String {
        self.read().display()
    }

    pub fn infix(
        &self,
        op: &str,
        other: &ValueHandle,
        ctx: &mut InterpreterContext,
    ) -> Result<ValueHandle> {
        match (self, other) {
            (Self::Ref(a), Self::Ref(b)) => a.read().infix(op, &b.read(), ctx),
            (Self::Ref(a), Self::Mut(b)) => a.read().infix(op, &b.read(), ctx),
            (Self::Mut(a), Self::Ref(b)) => a.write().infix_mut(op, &b.read(), ctx),
            (Self::Mut(a), Self::Mut(b)) => a.write().infix_mut(op, &b.read(), ctx),
        }
    }
}

// impl Drop for ValueHandle {
//     fn drop(&mut self) {
//         match self {
//             Self::Ref(lock) => {
//                 if lock.strong_count() == 1 {
//                     lock.read().despawn();
//                 }
//             }
//             Self::Mut(lock) => {
//                 if lock.strong_count() == 1 {
//                     lock.read().despawn();
//                 }
//             }
//         }
//     }
// }
