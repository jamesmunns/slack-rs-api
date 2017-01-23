use std::collections::HashMap;

use inflector::Inflector;

#[derive(Deserialize, Clone, Debug)]
pub struct JsonSchema {
    pub id: Option<String>,
    #[serde(rename = "$schema")]
    pub schema_ref: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub ty: Option<String>,
    pub properties: Option<HashMap<String, JsonSchema>>,
    pub required: Option<Vec<String>>,
    pub definitions: Option<HashMap<String, JsonSchema>>,
    pub items: Option<Box<JsonSchema>>,
    #[serde(rename = "patternProperties")]
    pub pattern_properties: Option<HashMap<String, JsonSchema>>,
    #[serde(default, rename = "additionalProperties")]
    pub additional_properties: bool,
    #[serde(rename = "$ref")]
    pub definition_ref: Option<String>,
    #[serde(rename = "oneOf")]
    pub one_of: Option<Vec<JsonSchema>>,
}

#[derive(Clone, Debug)]
pub struct JsonObject {
    pub name: String,
    pub fields: Vec<JsonObjectFieldInfo>,
}

#[derive(Clone, Debug)]
pub struct JsonObjectFieldInfo {
    pub name: String,
    pub ty: PropType,
    pub rename: Option<String>,
}

#[derive(Clone, Debug)]
pub struct JsonEnum {
    pub name: String,
    pub variants: Vec<JsonEnumVariant>,
}

#[derive(Clone, Debug)]
pub struct JsonEnumVariant {
    pub name: String,
    pub inner: PropType,
}

#[derive(Clone, Debug)]
pub enum PropType {
    Str,
    Int,
    Num,
    Bool,
    Ref(String),
    Obj(JsonObject),
    Arr(Box<PropType>),
    Map(Box<PropType>),
    Optional(Box<PropType>),
    Enum(JsonEnum),
    Null,
}

impl PropType {
    pub fn from_schema(schema: &JsonSchema, name: &str) -> Self {
        if let Some(ref def) = schema.definition_ref {
            return PropType::Ref(def.split("#/")
                .last()
                .and_then(|x| x.split('.').next())
                .unwrap()
                .to_pascal_case());
        }

        if let Some(ref one_of) = schema.one_of {
            return PropType::Enum(JsonEnum {
                name: name.to_owned(),
                variants: one_of.iter()
                    .map(|o| {
                        let variant_name = name.to_owned() +
                                           &o.id.as_ref().map(|s| s.to_pascal_case()).unwrap();
                        JsonEnumVariant {
                            name: variant_name.clone(),
                            inner: Self::from_schema(o, &variant_name),
                        }
                    })
                    .collect(),
            });
        }

        match schema.ty.as_ref().map(String::as_ref) {
            Some("boolean") => PropType::Bool,
            Some("string") => PropType::Str,
            Some("integer") => PropType::Int,
            Some("number") => PropType::Num,
            Some("null") => PropType::Null,
            Some("array") => {
                let item_name = &name.to_singular();
                if let Some(ref item_schema) = schema.items {
                    let subobj = Self::from_schema(&item_schema.clone(), &item_name);
                    PropType::Arr(Box::new(subobj))
                } else {
                    panic!("{} is an array but no schema is set for items", &item_name);
                }
            }
            Some("object") => {
                if let Some(ref pp) = schema.pattern_properties {
                    let subobj_schema = pp.iter().next().unwrap().1;
                    let subobj = Self::from_schema(&subobj_schema, &name);
                    PropType::Map(Box::new(subobj))
                } else {
                    PropType::Obj(schema.properties
                        .clone()
                        .map(|p| {
                            if p.is_empty() {
                                println!("{} is an object but has no properties. Likely an error.",
                                         &name);
                            }
                            let fields = p.iter()
                                .map(|(orig_name, p)| {
                                    let (field_name, rename) = match &orig_name[..] {
                                        "type" => ("ty", Some("type".into())),
                                        "self" => ("slf", Some("self".into())),
                                        name => (name, None),
                                    };
                                    let field_ty_name = name.to_owned() +
                                                        &orig_name.to_pascal_case();
                                    let mut ty = Self::from_schema(&p, &field_ty_name);
                                    if let Some(ref req) = schema.required {
                                        if !req.contains(orig_name) {
                                            ty = PropType::Optional(Box::new(ty));
                                        }
                                    } else {
                                        ty = PropType::Optional(Box::new(ty));
                                    }
                                    JsonObjectFieldInfo {
                                        name: field_name.into(),
                                        ty: ty,
                                        rename: rename,
                                    }
                                })
                                .collect();

                            JsonObject {
                                name: name.to_owned(),
                                fields: fields,
                            }
                        })
                        .expect("Failed to get sub object for object"))
                }
            }
            ty => panic!("Unknown JSON type: {:?} for {}", ty, name),
        }
    }

    pub fn to_rs_type(&self) -> String {
        match *self {
            PropType::Str => "String".into(),
            PropType::Int => "i32".into(),
            PropType::Num => "f32".into(),
            PropType::Bool => "bool".into(),
            PropType::Obj(ref obj) => obj.name.clone(),
            PropType::Ref(ref name) => format!("::{}", name),
            PropType::Arr(ref prop) => format!("Vec<{}>", prop.to_rs_type()),
            PropType::Map(ref prop) => format!("HashMap<String, {}>", prop.to_rs_type()),
            PropType::Optional(ref prop) => format!("Option<{}>", prop.to_rs_type()),
            PropType::Enum(ref e) => e.name.clone(),
            PropType::Null => "()".into(),
        }
    }
}