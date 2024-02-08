use fabricate::component::Atom;

pub trait Value: Atom {
    fn value_type() -> ValueType
    where
        Self: Sized;
    fn as_value_ref(&mut self, name: &'static str) -> ValueRef<'_>
    where
        Self: Sized,
    {
        ValueRef {
            name,
            typ: Self::value_type(),
            value: self,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ValueType {
    Bool,
    Int,
    Float,
    String,
    Vec3,
    Vec4,
    Color,
}

pub struct ValueRef<'a> {
    pub name: &'static str,
    pub typ: ValueType,
    pub value: &'a mut dyn Value,
}

pub trait Inspect {
    fn get_values(&mut self) -> Vec<ValueRef<'_>>;
    fn value_names(&self) -> Vec<&'static str>;
    fn value(&mut self, name: &str) -> Option<ValueRef<'_>>;
}

macro_rules! value_type {
    ($value:ident; $($ty:ty),*) => {
        $(
            impl Value for $ty {
                fn value_type() -> ValueType {
                    ValueType::$value
                }
            }
        )*
    };
}

value_type!(Bool; bool);
value_type!(Int; i32);
value_type!(Float; f32);
value_type!(String; String);
value_type!(Vec3; glam::Vec3);
value_type!(Vec4; glam::Vec4, glam::Quat);
value_type!(Color; crate::color::Color);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::approx_constant)]
    fn test_inspect() {
        struct Test {
            a: i32,
            b: f32,
            c: String,
        }

        impl Inspect for Test {
            fn get_values(&mut self) -> Vec<ValueRef> {
                vec![
                    ValueRef {
                        name: "a",
                        typ: ValueType::Int,
                        value: &mut self.a,
                    },
                    ValueRef {
                        name: "b",
                        typ: ValueType::Float,
                        value: &mut self.b,
                    },
                    ValueRef {
                        name: "c",
                        typ: ValueType::String,
                        value: &mut self.c,
                    },
                ]
            }

            fn value_names(&self) -> Vec<&'static str> {
                vec!["a", "b", "c"]
            }

            fn value(&mut self, name: &str) -> Option<ValueRef> {
                match name {
                    "a" => Some(ValueRef {
                        name: "a",
                        typ: ValueType::Int,
                        value: &mut self.a,
                    }),
                    "b" => Some(ValueRef {
                        name: "b",
                        typ: ValueType::Float,
                        value: &mut self.b,
                    }),
                    "c" => Some(ValueRef {
                        name: "c",
                        typ: ValueType::String,
                        value: &mut self.c,
                    }),
                    _ => None,
                }
            }
        }

        let mut test = Test {
            a: 42,
            b: 3.14,
            c: "hello".to_string(),
        };

        let values = test.get_values();
        assert_eq!(values.len(), 3);
        assert_eq!(values[0].name, "a");
        assert_eq!(values[0].typ, ValueType::Int);
        assert_eq!(*values[0].value.as_any().downcast_ref::<i32>().unwrap(), 42);
        assert_eq!(values[1].name, "b");
        assert_eq!(values[1].typ, ValueType::Float);
        assert_eq!(
            *values[1].value.as_any().downcast_ref::<f32>().unwrap(),
            3.14
        );
        assert_eq!(values[2].name, "c");
        assert_eq!(values[2].typ, ValueType::String);
        assert_eq!(
            *values[2].value.as_any().downcast_ref::<String>().unwrap(),
            "hello"
        );

        let names = test.value_names();
        assert_eq!(names.len(), 3);
        assert_eq!(names[0], "a");
        assert_eq!(names[1], "b");
        assert_eq!(names[2], "c");

        let value = test.value("a").unwrap();
        assert_eq!(value.name, "a");
        assert_eq!(value.typ, ValueType::Int);
        assert_eq!(*value.value.as_any().downcast_ref::<i32>().unwrap(), 42);

        let value = test.value("b").unwrap();
        assert_eq!(value.name, "b");
        assert_eq!(value.typ, ValueType::Float);
        assert_eq!(*value.value.as_any().downcast_ref::<f32>().unwrap(), 3.14);

        let value = test.value("c").unwrap();
        assert_eq!(value.name, "c");
        assert_eq!(value.typ, ValueType::String);
        assert_eq!(
            value.value.as_any().downcast_ref::<String>().unwrap(),
            "hello"
        );

        let value = test.value("d");
        assert!(value.is_none());
    }
}
