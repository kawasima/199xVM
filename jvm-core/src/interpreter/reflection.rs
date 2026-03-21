use std::rc::Rc;

use crate::class_file::Attribute;
use crate::heap::{JObject, JRef, JValue, NativePayload};

use super::{ReflectConstructorInfo, ReflectFieldInfo, ReflectMethodInfo, Vm};

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
            self.new_vm_exception("java/lang/RuntimeException", Some(JObject::new_string(message)))
        });
        let ite = self.new_vm_exception("java/lang/reflect/InvocationTargetException", None);
        ite.borrow_mut().fields.insert("cause".to_owned(), JValue::Ref(None));
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

    pub(super) fn build_reflect_field_from_info(
        &mut self,
        owner: &str,
        info: &ReflectFieldInfo,
    ) -> JRef {
        let obj = JObject::new("java/lang/reflect/Field");
        {
            let mut o = obj.borrow_mut();
            o.fields.insert(
                "clazz".to_owned(),
                JValue::Ref(Some(self.class_object(owner.to_owned()))),
            );
            o.fields.insert(
                "name".to_owned(),
                JValue::Ref(Some(self.intern_string(info.name.clone()))),
            );
            o.fields.insert(
                "__descriptor".to_owned(),
                JValue::Ref(Some(self.intern_string(info.descriptor.clone()))),
            );
            o.fields.insert(
                "type".to_owned(),
                JValue::Ref(Some(self.class_object(info.type_name.clone()))),
            );
            o.fields
                .insert("modifiers".to_owned(), JValue::Int(i32::from(info.access_flags)));
        }
        obj
    }

    pub(super) fn build_reflect_method_from_info(
        &mut self,
        owner: &str,
        info: &ReflectMethodInfo,
    ) -> JRef {
        let param_array = self.make_class_array(info.param_types.clone());
        let ex_array = self.make_class_array(info.exception_types.clone());
        let obj = JObject::new("java/lang/reflect/Method");
        {
            let mut o = obj.borrow_mut();
            o.fields.insert(
                "clazz".to_owned(),
                JValue::Ref(Some(self.class_object(owner.to_owned()))),
            );
            o.fields.insert(
                "name".to_owned(),
                JValue::Ref(Some(self.intern_string(info.name.clone()))),
            );
            o.fields.insert(
                "__descriptor".to_owned(),
                JValue::Ref(Some(self.intern_string(info.descriptor.clone()))),
            );
            o.fields.insert(
                "returnType".to_owned(),
                JValue::Ref(Some(self.class_object(info.return_type.clone()))),
            );
            o.fields
                .insert("parameterTypes".to_owned(), JValue::Ref(Some(param_array)));
            o.fields
                .insert("exceptionTypes".to_owned(), JValue::Ref(Some(ex_array)));
            o.fields
                .insert("modifiers".to_owned(), JValue::Int(i32::from(info.access_flags)));
        }
        obj
    }

    pub(super) fn build_reflect_constructor_from_info(
        &mut self,
        owner: &str,
        info: &ReflectConstructorInfo,
    ) -> JRef {
        let param_array = self.make_class_array(info.param_types.clone());
        let ex_array = self.make_class_array(info.exception_types.clone());
        let obj = JObject::new("java/lang/reflect/Constructor");
        {
            let mut o = obj.borrow_mut();
            o.fields.insert(
                "clazz".to_owned(),
                JValue::Ref(Some(self.class_object(owner.to_owned()))),
            );
            o.fields.insert(
                "__descriptor".to_owned(),
                JValue::Ref(Some(self.intern_string(info.descriptor.clone()))),
            );
            o.fields
                .insert("parameterTypes".to_owned(), JValue::Ref(Some(param_array)));
            o.fields
                .insert("exceptionTypes".to_owned(), JValue::Ref(Some(ex_array)));
            o.fields
                .insert("modifiers".to_owned(), JValue::Int(i32::from(info.access_flags)));
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

    pub(super) fn declared_field_infos(&mut self, class_name: &str) -> Rc<Vec<ReflectFieldInfo>> {
        if let Some(cached) = self.reflection_fields_cache.get(class_name) {
            return cached.clone();
        }
        self.ensure_class_ready(class_name);
        let infos: Vec<ReflectFieldInfo> = self
            .get_class(class_name)
            .map(|cf| {
                cf.fields
                    .iter()
                    .map(|f| {
                        let descriptor = cf.constant_pool.utf8(f.descriptor_index).to_owned();
                        ReflectFieldInfo {
                            name: cf.constant_pool.utf8(f.name_index).to_owned(),
                            type_name: Self::descriptor_to_runtime_class_name(&descriptor),
                            descriptor,
                            access_flags: f.access_flags,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        let infos = Rc::new(infos);
        self.reflection_fields_cache
            .insert(class_name.to_owned(), infos.clone());
        infos
    }

    pub(super) fn declared_method_infos(
        &mut self,
        class_name: &str,
    ) -> Rc<Vec<ReflectMethodInfo>> {
        if let Some(cached) = self.reflection_methods_cache.get(class_name) {
            return cached.clone();
        }
        self.ensure_class_ready(class_name);
        let infos: Vec<ReflectMethodInfo> = self
            .get_class(class_name)
            .map(|cf| {
                cf.methods
                    .iter()
                    .filter_map(|m| {
                        let name = cf.constant_pool.utf8(m.name_index);
                        if name == "<init>" || name == "<clinit>" {
                            return None;
                        }
                        let descriptor = cf.constant_pool.utf8(m.descriptor_index).to_owned();
                        let (param_types, return_type) = Self::parse_method_descriptor(&descriptor);
                        let exception_types = m
                            .attributes
                            .iter()
                            .find_map(|attr| match attr {
                                Attribute::Exceptions {
                                    exception_index_table,
                                } => Some(
                                    exception_index_table
                                        .iter()
                                        .map(|idx| cf.constant_pool.class_name(*idx).to_owned())
                                        .collect(),
                                ),
                                _ => None,
                            })
                            .unwrap_or_default();
                        Some(ReflectMethodInfo {
                            name: name.to_owned(),
                            descriptor,
                            param_types,
                            return_type,
                            exception_types,
                            access_flags: m.access_flags,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let infos = Rc::new(infos);
        self.reflection_methods_cache
            .insert(class_name.to_owned(), infos.clone());
        infos
    }

    pub(super) fn declared_constructor_infos(
        &mut self,
        class_name: &str,
    ) -> Rc<Vec<ReflectConstructorInfo>> {
        if let Some(cached) = self.reflection_ctors_cache.get(class_name) {
            return cached.clone();
        }
        self.ensure_class_ready(class_name);
        let infos: Vec<ReflectConstructorInfo> = self
            .get_class(class_name)
            .map(|cf| {
                cf.methods
                    .iter()
                    .filter_map(|m| {
                        if cf.constant_pool.utf8(m.name_index) != "<init>" {
                            return None;
                        }
                        let descriptor = cf.constant_pool.utf8(m.descriptor_index).to_owned();
                        let (param_types, _) = Self::parse_method_descriptor(&descriptor);
                        let exception_types = m
                            .attributes
                            .iter()
                            .find_map(|attr| match attr {
                                Attribute::Exceptions {
                                    exception_index_table,
                                } => Some(
                                    exception_index_table
                                        .iter()
                                        .map(|idx| cf.constant_pool.class_name(*idx).to_owned())
                                        .collect(),
                                ),
                                _ => None,
                            })
                            .unwrap_or_default();
                        Some(ReflectConstructorInfo {
                            descriptor,
                            param_types,
                            exception_types,
                            access_flags: m.access_flags,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let infos = Rc::new(infos);
        self.reflection_ctors_cache
            .insert(class_name.to_owned(), infos.clone());
        infos
    }
}
