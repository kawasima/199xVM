use crate::class_file::{Attribute, ConstantPoolEntry};
use crate::heap::{JObject, JRef, JValue};

use super::Vm;

impl Vm {
    fn read_u8(data: &[u8], p: &mut usize) -> Option<u8> {
        let b = *data.get(*p)?;
        *p += 1;
        Some(b)
    }

    fn read_u16_checked(data: &[u8], p: &mut usize) -> Option<u16> {
        let hi = *data.get(*p)? as u16;
        let lo = *data.get(*p + 1)? as u16;
        *p += 2;
        Some((hi << 8) | lo)
    }

    fn skip_annotation_element_value(data: &[u8], p: &mut usize) -> Option<()> {
        let tag = Self::read_u8(data, p)?;
        match tag as char {
            'B' | 'C' | 'D' | 'F' | 'I' | 'J' | 'S' | 'Z' | 's' => {
                let _ = Self::read_u16_checked(data, p)?;
            }
            'e' => {
                let _ = Self::read_u16_checked(data, p)?;
                let _ = Self::read_u16_checked(data, p)?;
            }
            'c' => {
                let _ = Self::read_u16_checked(data, p)?;
            }
            '@' => {
                Self::skip_annotation(data, p)?;
            }
            '[' => {
                let n = Self::read_u16_checked(data, p)? as usize;
                for _ in 0..n {
                    Self::skip_annotation_element_value(data, p)?;
                }
            }
            _ => return None,
        }
        Some(())
    }

    fn skip_annotation(data: &[u8], p: &mut usize) -> Option<()> {
        let _type_index = Self::read_u16_checked(data, p)?;
        let pairs = Self::read_u16_checked(data, p)? as usize;
        for _ in 0..pairs {
            let _name_index = Self::read_u16_checked(data, p)?;
            Self::skip_annotation_element_value(data, p)?;
        }
        Some(())
    }

    pub(super) fn parse_runtime_visible_annotation_types(
        attrs: &[Attribute],
        cp: &crate::class_file::ConstantPool,
    ) -> Vec<String> {
        let mut types = Vec::new();
        for attr in attrs {
            let (name, data) = match attr {
                Attribute::Unknown { name, data } => (name, data),
                _ => continue,
            };
            if name != "RuntimeVisibleAnnotations" {
                continue;
            }
            let mut p = 0usize;
            let n = match Self::read_u16_checked(data, &mut p) {
                Some(v) => v as usize,
                None => continue,
            };
            for _ in 0..n {
                let type_index = match Self::read_u16_checked(data, &mut p) {
                    Some(v) => v,
                    None => break,
                };
                let desc = cp.utf8(type_index).to_owned();
                if let Some(inner) = desc.strip_prefix('L').and_then(|s| s.strip_suffix(';')) {
                    types.push(inner.to_owned());
                }
                let pairs = match Self::read_u16_checked(data, &mut p) {
                    Some(v) => v as usize,
                    None => break,
                };
                for _ in 0..pairs {
                    if Self::read_u16_checked(data, &mut p).is_none() {
                        break;
                    }
                    if Self::skip_annotation_element_value(data, &mut p).is_none() {
                        break;
                    }
                }
            }
        }
        types
    }

    pub(super) fn cp_const_to_jvalue(&mut self, cp: &crate::class_file::ConstantPool, idx: u16) -> Option<JValue> {
        match cp.get(idx) {
            ConstantPoolEntry::Integer(v) => Some(JValue::Int(*v)),
            ConstantPoolEntry::Long(v) => Some(JValue::Long(*v)),
            ConstantPoolEntry::Float(v) => Some(JValue::Float(*v)),
            ConstantPoolEntry::Double(v) => Some(JValue::Double(*v)),
            ConstantPoolEntry::String { string_index } => {
                let s = cp.utf8(*string_index).to_owned();
                Some(JValue::Ref(Some(self.intern_string(s))))
            }
            ConstantPoolEntry::Utf8(s) => Some(JValue::Ref(Some(self.intern_string(s.clone())))),
            _ => None,
        }
    }

    pub(super) fn parse_annotation_element_value(
        &mut self,
        data: &[u8],
        p: &mut usize,
        cp: &crate::class_file::ConstantPool,
    ) -> Option<JValue> {
        let tag = Self::read_u8(data, p)? as char;
        match tag {
            'B' | 'C' | 'D' | 'F' | 'I' | 'J' | 'S' | 'Z' | 's' => {
                let const_idx = Self::read_u16_checked(data, p)?;
                self.cp_const_to_jvalue(cp, const_idx)
            }
            'e' => {
                let type_name_index = Self::read_u16_checked(data, p)?;
                let const_name_index = Self::read_u16_checked(data, p)?;
                let t = cp.utf8(type_name_index);
                let c = cp.utf8(const_name_index);
                Some(JValue::Ref(Some(self.intern_string(format!("{t}.{c}")))))
            }
            'c' => {
                let class_info_index = Self::read_u16_checked(data, p)?;
                let desc = cp.utf8(class_info_index);
                Some(JValue::Ref(Some(self.class_object(Self::descriptor_to_runtime_class_name(desc)))))
            }
            '@' => {
                let ann = self.parse_one_runtime_visible_annotation(data, p, cp)?;
                Some(JValue::Ref(Some(ann)))
            }
            '[' => {
                let n = Self::read_u16_checked(data, p)? as usize;
                let mut vals = Vec::with_capacity(n);
                for _ in 0..n {
                    vals.push(self.parse_annotation_element_value(data, p, cp).unwrap_or(JValue::Ref(None)));
                }
                Some(JValue::Ref(Some(JObject::new_array("[Ljava/lang/Object;", vals))))
            }
            _ => None,
        }
    }

    pub(super) fn parse_one_runtime_visible_annotation(
        &mut self,
        data: &[u8],
        p: &mut usize,
        cp: &crate::class_file::ConstantPool,
    ) -> Option<JRef> {
        let type_index = Self::read_u16_checked(data, p)?;
        let desc = cp.utf8(type_index).to_owned();
        let ann_class = desc.strip_prefix('L')?.strip_suffix(';')?.to_owned();
        let pairs = Self::read_u16_checked(data, p)? as usize;
        let ann_obj = JObject::new(ann_class.clone());
        {
            let mut o = ann_obj.borrow_mut();
            o.fields.insert(
                "__ann_type".to_owned(),
                JValue::Ref(Some(self.class_object(ann_class))),
            );
        }
        for _ in 0..pairs {
            let name_index = Self::read_u16_checked(data, p)?;
            let name = cp.utf8(name_index).to_owned();
            let value = self
                .parse_annotation_element_value(data, p, cp)
                .unwrap_or(JValue::Ref(None));
            ann_obj
                .borrow_mut()
                .fields
                .insert(format!("__ann_{name}"), value);
        }
        Some(ann_obj)
    }

    pub(super) fn parse_runtime_visible_annotations(
        &mut self,
        attrs: &[Attribute],
        cp: &crate::class_file::ConstantPool,
    ) -> Vec<JRef> {
        let mut anns = Vec::new();
        for attr in attrs {
            let (name, data) = match attr {
                Attribute::Unknown { name, data } => (name, data),
                _ => continue,
            };
            if name != "RuntimeVisibleAnnotations" {
                continue;
            }
            let mut p = 0usize;
            let n = match Self::read_u16_checked(data, &mut p) {
                Some(v) => v as usize,
                None => continue,
            };
            for _ in 0..n {
                if let Some(ann) = self.parse_one_runtime_visible_annotation(data, &mut p, cp) {
                    anns.push(ann);
                } else {
                    break;
                }
            }
        }
        anns
    }

    pub(super) fn parse_runtime_visible_parameter_annotation_types(
        attrs: &[Attribute],
        cp: &crate::class_file::ConstantPool,
        param_count: usize,
    ) -> Vec<Vec<String>> {
        let mut out = vec![Vec::new(); param_count];
        for attr in attrs {
            let (name, data) = match attr {
                Attribute::Unknown { name, data } => (name, data),
                _ => continue,
            };
            if name != "RuntimeVisibleParameterAnnotations" {
                continue;
            }
            let mut p = 0usize;
            let declared = match Self::read_u8(data, &mut p) {
                Some(v) => v as usize,
                None => continue,
            };
            for i in 0..declared {
                let n = match Self::read_u16_checked(data, &mut p) {
                    Some(v) => v as usize,
                    None => break,
                };
                let mut ann_types = Vec::new();
                for _ in 0..n {
                    let type_index = match Self::read_u16_checked(data, &mut p) {
                        Some(v) => v,
                        None => break,
                    };
                    let desc = cp.utf8(type_index).to_owned();
                    if let Some(inner) = desc.strip_prefix('L').and_then(|s| s.strip_suffix(';')) {
                        ann_types.push(inner.to_owned());
                    }
                    let pairs = match Self::read_u16_checked(data, &mut p) {
                        Some(v) => v as usize,
                        None => break,
                    };
                    for _ in 0..pairs {
                        if Self::read_u16_checked(data, &mut p).is_none() {
                            break;
                        }
                        if Self::skip_annotation_element_value(data, &mut p).is_none() {
                            break;
                        }
                    }
                }
                if i < out.len() {
                    out[i] = ann_types;
                }
            }
        }
        out
    }

    pub(super) fn parse_runtime_visible_parameter_annotations(
        &mut self,
        attrs: &[Attribute],
        cp: &crate::class_file::ConstantPool,
        param_count: usize,
    ) -> Vec<Vec<JRef>> {
        let mut out = vec![Vec::new(); param_count];
        for attr in attrs {
            let (name, data) = match attr {
                Attribute::Unknown { name, data } => (name, data),
                _ => continue,
            };
            if name != "RuntimeVisibleParameterAnnotations" {
                continue;
            }
            let mut p = 0usize;
            let declared = match Self::read_u8(data, &mut p) {
                Some(v) => v as usize,
                None => continue,
            };
            for i in 0..declared {
                let n = match Self::read_u16_checked(data, &mut p) {
                    Some(v) => v as usize,
                    None => break,
                };
                let mut anns = Vec::new();
                for _ in 0..n {
                    if let Some(ann) = self.parse_one_runtime_visible_annotation(data, &mut p, cp) {
                        anns.push(ann);
                    } else {
                        break;
                    }
                }
                if i < out.len() {
                    out[i] = anns;
                }
            }
        }
        out
    }

    pub(super) fn build_annotation_array(&self, annotation_types: Vec<String>) -> JValue {
        let vals = annotation_types
            .into_iter()
            .map(|ann| JValue::Ref(Some(JObject::new(ann))))
            .collect();
        JValue::Ref(Some(JObject::new_array(
            "[Ljava/lang/annotation/Annotation;",
            vals,
        )))
    }

    pub(super) fn build_annotation_ref_array(&self, annotation_refs: Vec<JRef>) -> JValue {
        let vals = annotation_refs
            .into_iter()
            .map(|ann| JValue::Ref(Some(ann)))
            .collect();
        JValue::Ref(Some(JObject::new_array(
            "[Ljava/lang/annotation/Annotation;",
            vals,
        )))
    }
}
