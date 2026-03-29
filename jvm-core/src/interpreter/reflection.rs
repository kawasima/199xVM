use std::rc::Rc;

use crate::class_file::Attribute;
use crate::heap::{JObject, JRef, JValue, NativePayload};

use super::{ClassId, ReflectConstructorInfo, ReflectFieldInfo, ReflectMethodInfo, Vm};

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
            self.new_vm_exception(
                "java/lang/RuntimeException",
                Some(JObject::new_string(message)),
            )
        });
        let ite = self.new_vm_exception("java/lang/reflect/InvocationTargetException", None);
        self.set_object_field_value(&ite, "cause", JValue::Ref(None));
        self.set_object_field_value(&ite, "target", JValue::Ref(Some(cause)));
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
        let owner_class = self.class_object(owner.to_owned());
        let field_name = self.intern_string(info.name.clone());
        let descriptor = self.intern_string(info.descriptor.clone());
        let field_type = self.class_object(info.type_name.clone());
        self.set_object_field_value(
            &obj,
            "clazz",
            JValue::Ref(Some(owner_class)),
        );
        self.set_object_field_value(
            &obj,
            "name",
            JValue::Ref(Some(field_name)),
        );
        self.set_object_field_value(
            &obj,
            "__descriptor",
            JValue::Ref(Some(descriptor)),
        );
        self.set_object_field_value(
            &obj,
            "type",
            JValue::Ref(Some(field_type)),
        );
        self.set_object_field_value(
            &obj,
            "modifiers",
            JValue::Int(i32::from(info.access_flags)),
        );
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
        let owner_class = self.class_object(owner.to_owned());
        let method_name = self.intern_string(info.name.clone());
        let descriptor = self.intern_string(info.descriptor.clone());
        let return_type = self.class_object(info.return_type.clone());
        self.set_object_field_value(
            &obj,
            "clazz",
            JValue::Ref(Some(owner_class)),
        );
        self.set_object_field_value(
            &obj,
            "name",
            JValue::Ref(Some(method_name)),
        );
        self.set_object_field_value(
            &obj,
            "__descriptor",
            JValue::Ref(Some(descriptor)),
        );
        self.set_object_field_value(
            &obj,
            "returnType",
            JValue::Ref(Some(return_type)),
        );
        self.set_object_field_value(&obj, "parameterTypes", JValue::Ref(Some(param_array)));
        self.set_object_field_value(&obj, "exceptionTypes", JValue::Ref(Some(ex_array)));
        self.set_object_field_value(
            &obj,
            "modifiers",
            JValue::Int(i32::from(info.access_flags)),
        );
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
        let owner_class = self.class_object(owner.to_owned());
        let descriptor = self.intern_string(info.descriptor.clone());
        self.set_object_field_value(
            &obj,
            "clazz",
            JValue::Ref(Some(owner_class)),
        );
        self.set_object_field_value(
            &obj,
            "__descriptor",
            JValue::Ref(Some(descriptor)),
        );
        self.set_object_field_value(&obj, "parameterTypes", JValue::Ref(Some(param_array)));
        self.set_object_field_value(&obj, "exceptionTypes", JValue::Ref(Some(ex_array)));
        self.set_object_field_value(
            &obj,
            "modifiers",
            JValue::Int(i32::from(info.access_flags)),
        );
        obj
    }

    pub(super) fn build_reflect_record_component(
        &mut self,
        owner: &str,
        name: &str,
        desc: &str,
    ) -> JRef {
        let obj = JObject::new("java/lang/reflect/RecordComponent");
        let owner_class = self.class_object(owner.to_owned());
        let name_ref = self.intern_string(name.to_owned());
        let descriptor = self.intern_string(desc.to_owned());
        let type_ref = self.class_object(Self::descriptor_to_runtime_class_name(desc));
        self.set_object_field_value(
            &obj,
            "clazz",
            JValue::Ref(Some(owner_class)),
        );
        self.set_object_field_value(
            &obj,
            "name",
            JValue::Ref(Some(name_ref)),
        );
        self.set_object_field_value(
            &obj,
            "__descriptor",
            JValue::Ref(Some(descriptor)),
        );
        self.set_object_field_value(
            &obj,
            "type",
            JValue::Ref(Some(type_ref)),
        );
        obj
    }

    pub(super) fn declared_field_infos(&mut self, class_name: &str) -> Rc<Vec<ReflectFieldInfo>> {
        let Some(class_id) = self.tracked_class_id_for_name(class_name) else {
            return Rc::new(Vec::new());
        };
        self.declared_field_infos_id(class_id)
    }

    pub(super) fn declared_field_infos_id(
        &mut self,
        class_id: ClassId,
    ) -> Rc<Vec<ReflectFieldInfo>> {
        if let Some(cached) = self.reflection_fields_cache.get(&class_id) {
            return cached.clone();
        }
        let _ = self.ensure_class_prepared(class_id);
        let infos: Vec<ReflectFieldInfo> = self
            .parsed_class(class_id)
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
            .insert(class_id, infos.clone());
        infos
    }

    pub(super) fn declared_method_infos(&mut self, class_name: &str) -> Rc<Vec<ReflectMethodInfo>> {
        let Some(class_id) = self.tracked_class_id_for_name(class_name) else {
            return Rc::new(Vec::new());
        };
        self.declared_method_infos_id(class_id)
    }

    pub(super) fn declared_method_infos_id(
        &mut self,
        class_id: ClassId,
    ) -> Rc<Vec<ReflectMethodInfo>> {
        if let Some(cached) = self.reflection_methods_cache.get(&class_id) {
            return cached.clone();
        }
        let _ = self.ensure_class_prepared(class_id);
        let infos: Vec<ReflectMethodInfo> = self
            .parsed_class(class_id)
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
            .insert(class_id, infos.clone());
        infos
    }

    pub(super) fn declared_constructor_infos(
        &mut self,
        class_name: &str,
    ) -> Rc<Vec<ReflectConstructorInfo>> {
        let Some(class_id) = self.tracked_class_id_for_name(class_name) else {
            return Rc::new(Vec::new());
        };
        self.declared_constructor_infos_id(class_id)
    }

    pub(super) fn declared_constructor_infos_id(
        &mut self,
        class_id: ClassId,
    ) -> Rc<Vec<ReflectConstructorInfo>> {
        if let Some(cached) = self.reflection_ctors_cache.get(&class_id) {
            return cached.clone();
        }
        let _ = self.ensure_class_prepared(class_id);
        let infos: Vec<ReflectConstructorInfo> = self
            .parsed_class(class_id)
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
            .insert(class_id, infos.clone());
        infos
    }
}
