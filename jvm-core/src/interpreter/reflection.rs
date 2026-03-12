use crate::heap::{JObject, JRef, JValue, NativePayload};

use super::Vm;

impl Vm {
    pub(super) fn collect_reflection_args(&self, args_array: Option<&JRef>) -> Vec<JValue> {
        if let Some(arr) = args_array {
            if let NativePayload::Array(v) = &arr.borrow().native {
                return v.clone();
            }
        }
        Vec::new()
    }

    pub(super) fn raise_invocation_target_exception(&mut self, message: &str) {
        let cause = self.pending_exception_mut().take().unwrap_or_else(|| {
            let exc = JObject::new("java/lang/RuntimeException");
            exc.borrow_mut().fields.insert(
                "detailMessage".to_owned(),
                JValue::Ref(Some(JObject::new_string(message.to_owned()))),
            );
            exc
        });
        let ite = JObject::new("java/lang/reflect/InvocationTargetException");
        ite.borrow_mut().fields.insert("target".to_owned(), JValue::Ref(Some(cause)));
        *self.pending_exception_mut() = Some(ite);
    }

    pub(super) fn make_class_array(&mut self, class_names: Vec<String>) -> JRef {
        let values = class_names
            .into_iter()
            .map(|n| JValue::Ref(Some(self.class_object(n))))
            .collect();
        JObject::new_array("[Ljava/lang/Class;", values)
    }

    pub(super) fn build_reflect_field(&mut self, owner: &str, name: &str, desc: &str, access_flags: u16) -> JRef {
        let obj = JObject::new("java/lang/reflect/Field");
        {
            let mut o = obj.borrow_mut();
            o.fields.insert("clazz".to_owned(), JValue::Ref(Some(self.class_object(owner.to_owned()))));
            o.fields.insert("name".to_owned(), JValue::Ref(Some(self.intern_string(name.to_owned()))));
            o.fields.insert("__descriptor".to_owned(), JValue::Ref(Some(self.intern_string(desc.to_owned()))));
            o.fields.insert(
                "type".to_owned(),
                JValue::Ref(Some(self.class_object(Self::descriptor_to_runtime_class_name(desc)))),
            );
            o.fields.insert("modifiers".to_owned(), JValue::Int(i32::from(access_flags)));
        }
        obj
    }

    pub(super) fn build_reflect_method(
        &mut self,
        owner: &str,
        name: &str,
        method_desc: &str,
        access_flags: u16,
        exceptions: Vec<String>,
    ) -> JRef {
        let (param_names, return_name) = Self::parse_method_descriptor(method_desc);
        let param_array = self.make_class_array(param_names);
        let ex_array = self.make_class_array(exceptions);
        let obj = JObject::new("java/lang/reflect/Method");
        {
            let mut o = obj.borrow_mut();
            o.fields.insert("clazz".to_owned(), JValue::Ref(Some(self.class_object(owner.to_owned()))));
            o.fields.insert("name".to_owned(), JValue::Ref(Some(self.intern_string(name.to_owned()))));
            o.fields.insert("__descriptor".to_owned(), JValue::Ref(Some(self.intern_string(method_desc.to_owned()))));
            o.fields.insert(
                "returnType".to_owned(),
                JValue::Ref(Some(self.class_object(return_name))),
            );
            o.fields
                .insert("parameterTypes".to_owned(), JValue::Ref(Some(param_array)));
            o.fields
                .insert("exceptionTypes".to_owned(), JValue::Ref(Some(ex_array)));
            o.fields.insert("modifiers".to_owned(), JValue::Int(i32::from(access_flags)));
        }
        obj
    }

    pub(super) fn build_reflect_constructor(
        &mut self,
        owner: &str,
        method_desc: &str,
        access_flags: u16,
        exceptions: Vec<String>,
    ) -> JRef {
        let (param_names, _) = Self::parse_method_descriptor(method_desc);
        let param_array = self.make_class_array(param_names);
        let ex_array = self.make_class_array(exceptions);
        let obj = JObject::new("java/lang/reflect/Constructor");
        {
            let mut o = obj.borrow_mut();
            o.fields.insert("clazz".to_owned(), JValue::Ref(Some(self.class_object(owner.to_owned()))));
            o.fields.insert("__descriptor".to_owned(), JValue::Ref(Some(self.intern_string(method_desc.to_owned()))));
            o.fields
                .insert("parameterTypes".to_owned(), JValue::Ref(Some(param_array)));
            o.fields
                .insert("exceptionTypes".to_owned(), JValue::Ref(Some(ex_array)));
            o.fields.insert("modifiers".to_owned(), JValue::Int(i32::from(access_flags)));
        }
        obj
    }

    pub(super) fn build_reflect_record_component(&mut self, owner: &str, name: &str, desc: &str) -> JRef {
        let obj = JObject::new("java/lang/reflect/RecordComponent");
        {
            let mut o = obj.borrow_mut();
            o.fields.insert("clazz".to_owned(), JValue::Ref(Some(self.class_object(owner.to_owned()))));
            o.fields.insert("name".to_owned(), JValue::Ref(Some(self.intern_string(name.to_owned()))));
            o.fields.insert("__descriptor".to_owned(), JValue::Ref(Some(self.intern_string(desc.to_owned()))));
            o.fields.insert(
                "type".to_owned(),
                JValue::Ref(Some(self.class_object(Self::descriptor_to_runtime_class_name(desc)))),
            );
        }
        obj
    }
}
