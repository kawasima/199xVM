//! Java class file parser.
//!
//! Parses the binary `.class` format as specified in JVMS §4.
//! Supports class file versions up to 69 (Java 25).

use std::rc::Rc;

/// Magic number that starts every `.class` file.
const MAGIC: u32 = 0xCAFE_BABE;

/// A parsed Java class file.
#[derive(Debug)]
pub struct ClassFile {
    pub minor_version: u16,
    pub major_version: u16,
    pub constant_pool: ConstantPool,
    pub access_flags: u16,
    pub this_class: u16,
    pub super_class: u16,
    pub interfaces: Vec<u16>,
    pub fields: Vec<FieldInfo>,
    pub methods: Vec<MethodInfo>,
    pub attributes: Vec<Attribute>,
}

/// Constant pool wrapper with 1-based indexing (index 0 is unused per spec).
///
/// `entries` is wrapped in `Rc` so cloning the constant pool (e.g. when
/// passing it to `run_frame`) is O(1) instead of O(n).
#[derive(Debug, Clone)]
pub struct ConstantPool {
    pub(crate) entries: Rc<Vec<ConstantPoolEntry>>,
}

impl ConstantPool {
    fn new(entries: Vec<ConstantPoolEntry>) -> Self {
        Self { entries: Rc::new(entries) }
    }

    /// Get entry at 1-based index.
    pub fn get(&self, index: u16) -> &ConstantPoolEntry {
        &self.entries[index as usize]
    }

    /// Resolve a Utf8 entry to a `&str`.
    pub fn utf8(&self, index: u16) -> &str {
        match self.get(index) {
            ConstantPoolEntry::Utf8(s) => s,
            other => panic!("Expected Utf8 at cp[{index}], got {other:?}"),
        }
    }

    /// Resolve a Class entry to its name string.
    pub fn class_name(&self, index: u16) -> &str {
        match self.get(index) {
            ConstantPoolEntry::Class { name_index } => self.utf8(*name_index),
            other => panic!("Expected Class at cp[{index}], got {other:?}"),
        }
    }

    /// Resolve a NameAndType entry.
    pub fn name_and_type(&self, index: u16) -> (&str, &str) {
        match self.get(index) {
            ConstantPoolEntry::NameAndType { name_index, descriptor_index } => {
                (self.utf8(*name_index), self.utf8(*descriptor_index))
            }
            other => panic!("Expected NameAndType at cp[{index}], got {other:?}"),
        }
    }
}

/// A single constant pool entry (JVMS §4.4).
#[derive(Debug, Clone)]
pub enum ConstantPoolEntry {
    /// Placeholder for the second slot of Long/Double (spec §4.4.5).
    Placeholder,
    Utf8(String),
    Integer(i32),
    Float(f32),
    Long(i64),
    Double(f64),
    Class { name_index: u16 },
    String { string_index: u16 },
    Fieldref { class_index: u16, name_and_type_index: u16 },
    Methodref { class_index: u16, name_and_type_index: u16 },
    InterfaceMethodref { class_index: u16, name_and_type_index: u16 },
    NameAndType { name_index: u16, descriptor_index: u16 },
    MethodHandle { reference_kind: u8, reference_index: u16 },
    MethodType { descriptor_index: u16 },
    Dynamic { bootstrap_method_attr_index: u16, name_and_type_index: u16 },
    InvokeDynamic { bootstrap_method_attr_index: u16, name_and_type_index: u16 },
    Module { name_index: u16 },
    Package { name_index: u16 },
}

/// Field metadata.
#[derive(Debug)]
pub struct FieldInfo {
    pub access_flags: u16,
    pub name_index: u16,
    pub descriptor_index: u16,
    pub attributes: Vec<Attribute>,
}

/// Method metadata including bytecode.
#[derive(Debug)]
pub struct MethodInfo {
    pub access_flags: u16,
    pub name_index: u16,
    pub descriptor_index: u16,
    pub attributes: Vec<Attribute>,
}

impl MethodInfo {
    /// Extract the `Code` attribute if present.
    pub fn code(&self) -> Option<&CodeAttribute> {
        self.attributes.iter().find_map(|a| {
            if let Attribute::Code(code) = a {
                Some(code)
            } else {
                None
            }
        })
    }
}

/// A class/method/field attribute (JVMS §4.7).
#[derive(Debug, Clone)]
pub enum Attribute {
    Code(CodeAttribute),
    ConstantValue { constantvalue_index: u16 },
    BootstrapMethods(Vec<BootstrapMethod>),
    Exceptions { exception_index_table: Vec<u16> },
    LineNumberTable(Vec<LineNumberEntry>),
    LocalVariableTable(Vec<LocalVariableEntry>),
    SourceFile { sourcefile_index: u16 },
    Signature { signature_index: u16 },
    InnerClasses(Vec<InnerClassEntry>),
    EnclosingMethod { class_index: u16, method_index: u16 },
    NestHost { host_class_index: u16 },
    NestMembers { classes: Vec<u16> },
    PermittedSubclasses { classes: Vec<u16> },
    Record { components: Vec<RecordComponentInfo> },
    Unknown { name: String, data: Vec<u8> },
}

/// The `Code` attribute containing bytecode and exception table.
#[derive(Debug, Clone)]
pub struct CodeAttribute {
    pub max_stack: u16,
    pub max_locals: u16,
    pub code: Vec<u8>,
    pub exception_table: Vec<ExceptionTableEntry>,
    pub attributes: Vec<Attribute>,
}

/// One entry in an exception table.
#[derive(Debug, Clone)]
pub struct ExceptionTableEntry {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_pc: u16,
    pub catch_type: u16,
}

/// One bootstrap method referenced by `invokedynamic`.
#[derive(Debug, Clone)]
pub struct BootstrapMethod {
    pub bootstrap_method_ref: u16,
    pub bootstrap_arguments: Vec<u16>,
}

#[derive(Debug, Clone)]
pub struct LineNumberEntry {
    pub start_pc: u16,
    pub line_number: u16,
}

#[derive(Debug, Clone)]
pub struct LocalVariableEntry {
    pub start_pc: u16,
    pub length: u16,
    pub name_index: u16,
    pub descriptor_index: u16,
    pub index: u16,
}

#[derive(Debug, Clone)]
pub struct InnerClassEntry {
    pub inner_class_info_index: u16,
    pub outer_class_info_index: u16,
    pub inner_name_index: u16,
    pub inner_class_access_flags: u16,
}

#[derive(Debug, Clone)]
pub struct RecordComponentInfo {
    pub name_index: u16,
    pub descriptor_index: u16,
    pub attributes: Vec<Attribute>,
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn u8(&mut self) -> u8 {
        let b = self.data[self.pos];
        self.pos += 1;
        b
    }

    fn u16(&mut self) -> u16 {
        let hi = self.u8() as u16;
        let lo = self.u8() as u16;
        (hi << 8) | lo
    }

    fn u32(&mut self) -> u32 {
        let a = self.u16() as u32;
        let b = self.u16() as u32;
        (a << 16) | b
    }

    fn i32(&mut self) -> i32 {
        self.u32() as i32
    }

    fn i64(&mut self) -> i64 {
        let hi = self.u32() as i64;
        let lo = self.u32() as i64;
        (hi << 32) | lo
    }

    fn f32(&mut self) -> f32 {
        f32::from_bits(self.u32())
    }

    fn f64(&mut self) -> f64 {
        f64::from_bits((self.i64()) as u64)
    }

    fn bytes(&mut self, n: usize) -> Vec<u8> {
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        slice.to_vec()
    }
}

/// Extract only the internal class name from raw `.class` bytes without a full parse.
///
/// Reads: magic(4) + minor(2) + major(2) + cp_count(2), scans the constant pool,
/// then reads `this_class` to find the `Class` entry whose `name_index` points to
/// the `Utf8` entry containing the internal class name.
///
/// Utf8 entries are recorded as `(offset, length)` byte-range pointers into `data`
/// rather than decoded `String`s, so only the single target entry is ever decoded.
///
/// Returns `None` if the bytes are too short or malformed.
pub fn parse_class_name(data: &[u8]) -> Option<String> {
    // Bounds-checked read helpers.
    let read_u8 = |pos: &mut usize| -> Option<u8> {
        let b = *data.get(*pos)?;
        *pos += 1;
        Some(b)
    };
    let read_u16 = |pos: &mut usize| -> Option<u16> {
        let hi = *data.get(*pos)? as u16;
        let lo = *data.get(*pos + 1)? as u16;
        *pos += 2;
        Some((hi << 8) | lo)
    };
    let read_u32 = |pos: &mut usize| -> Option<u32> {
        let a = *data.get(*pos)? as u32;
        let b = *data.get(*pos + 1)? as u32;
        let c = *data.get(*pos + 2)? as u32;
        let d = *data.get(*pos + 3)? as u32;
        *pos += 4;
        Some((a << 24) | (b << 16) | (c << 8) | d)
    };

    let pos = &mut 0usize;

    // magic
    if read_u32(pos)? != 0xCAFEBABE { return None; }
    // minor + major
    read_u16(pos)?;
    read_u16(pos)?;
    // constant pool count
    let cp_count = read_u16(pos)? as usize;

    // Scan the constant pool.
    // Utf8 entries: record (byte_offset, byte_len) — no String allocation yet.
    // Class entries: record name_index.
    let mut utf8_spans: Vec<Option<(usize, usize)>> = vec![None; cp_count];
    let mut class_name_indices: Vec<Option<u16>> = vec![None; cp_count];
    let mut i = 1usize;
    while i < cp_count {
        let tag = read_u8(pos)?;
        match tag {
            1 => {
                // Utf8 — record span, skip bytes
                let len = read_u16(pos)? as usize;
                let start = *pos;
                let end = start.checked_add(len)?;
                if end > data.len() { return None; }
                utf8_spans[i] = Some((start, len));
                *pos = end;
                i += 1;
            }
            7 => {
                // Class — name_index
                class_name_indices[i] = Some(read_u16(pos)?);
                i += 1;
            }
            8 | 16 | 19 | 20 => {
                // String, MethodType, Module, Package — 2-byte index
                read_u16(pos)?;
                i += 1;
            }
            3 | 4 => {
                // Integer, Float — 4 bytes
                read_u32(pos)?;
                i += 1;
            }
            5 | 6 => {
                // Long, Double — 8 bytes; next slot is unusable (JVMS §4.4.5)
                read_u32(pos)?;
                read_u32(pos)?;
                i += 2;
            }
            9 | 10 | 11 | 12 => {
                // Fieldref, Methodref, InterfaceMethodref, NameAndType — 4 bytes
                read_u32(pos)?;
                i += 1;
            }
            15 => {
                // MethodHandle — 3 bytes
                read_u8(pos)?;
                read_u16(pos)?;
                i += 1;
            }
            17 | 18 => {
                // Dynamic, InvokeDynamic — 4 bytes
                read_u32(pos)?;
                i += 1;
            }
            _ => return None,
        }
    }

    // access_flags(2) + this_class(2)
    read_u16(pos)?; // access_flags
    let this_class = read_u16(pos)? as usize;
    if this_class >= cp_count { return None; }
    let name_index = class_name_indices[this_class]? as usize;
    if name_index >= cp_count { return None; }

    // Decode only the single target Utf8 entry.
    let (start, len) = utf8_spans[name_index]?;
    Some(String::from_utf8_lossy(&data[start..start + len]).into_owned())
}

/// Parse a `.class` file from raw bytes.
pub fn parse(data: &[u8]) -> Result<ClassFile, String> {
    let mut r = Reader::new(data);

    let magic = r.u32();
    if magic != MAGIC {
        return Err(format!("Invalid magic: 0x{magic:08X}"));
    }

    let minor_version = r.u16();
    let major_version = r.u16();

    // Constant pool: count includes index 0 which is unused.
    let cp_count = r.u16();
    let constant_pool = parse_constant_pool(&mut r, cp_count)?;

    let access_flags = r.u16();
    let this_class = r.u16();
    let super_class = r.u16();

    let interfaces_count = r.u16();
    let interfaces = (0..interfaces_count).map(|_| r.u16()).collect();

    let fields_count = r.u16();
    let fields = (0..fields_count)
        .map(|_| parse_field(&mut r, &constant_pool))
        .collect::<Result<Vec<_>, _>>()?;

    let methods_count = r.u16();
    let methods = (0..methods_count)
        .map(|_| parse_method(&mut r, &constant_pool))
        .collect::<Result<Vec<_>, _>>()?;

    let attributes_count = r.u16();
    let attributes = (0..attributes_count)
        .map(|_| parse_attribute(&mut r, &constant_pool))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ClassFile {
        minor_version,
        major_version,
        constant_pool,
        access_flags,
        this_class,
        super_class,
        interfaces,
        fields,
        methods,
        attributes,
    })
}

fn parse_constant_pool(r: &mut Reader, count: u16) -> Result<ConstantPool, String> {
    // Index 0 is unused; fill with Placeholder.
    let mut entries = vec![ConstantPoolEntry::Placeholder];
    let mut i = 1u16;
    while i < count {
        let tag = r.u8();
        let entry = match tag {
            1 => {
                let length = r.u16() as usize;
                let bytes = r.bytes(length);
                // Modified UTF-8 — for ASCII-heavy Java class names this is fine.
                let s = String::from_utf8_lossy(&bytes).into_owned();
                ConstantPoolEntry::Utf8(s)
            }
            3 => ConstantPoolEntry::Integer(r.i32()),
            4 => ConstantPoolEntry::Float(r.f32()),
            5 => {
                let v = ConstantPoolEntry::Long(r.i64());
                entries.push(v);
                entries.push(ConstantPoolEntry::Placeholder);
                i += 2;
                continue;
            }
            6 => {
                let v = ConstantPoolEntry::Double(r.f64());
                entries.push(v);
                entries.push(ConstantPoolEntry::Placeholder);
                i += 2;
                continue;
            }
            7 => ConstantPoolEntry::Class { name_index: r.u16() },
            8 => ConstantPoolEntry::String { string_index: r.u16() },
            9 => ConstantPoolEntry::Fieldref {
                class_index: r.u16(),
                name_and_type_index: r.u16(),
            },
            10 => ConstantPoolEntry::Methodref {
                class_index: r.u16(),
                name_and_type_index: r.u16(),
            },
            11 => ConstantPoolEntry::InterfaceMethodref {
                class_index: r.u16(),
                name_and_type_index: r.u16(),
            },
            12 => ConstantPoolEntry::NameAndType {
                name_index: r.u16(),
                descriptor_index: r.u16(),
            },
            15 => ConstantPoolEntry::MethodHandle {
                reference_kind: r.u8(),
                reference_index: r.u16(),
            },
            16 => ConstantPoolEntry::MethodType { descriptor_index: r.u16() },
            17 => ConstantPoolEntry::Dynamic {
                bootstrap_method_attr_index: r.u16(),
                name_and_type_index: r.u16(),
            },
            18 => ConstantPoolEntry::InvokeDynamic {
                bootstrap_method_attr_index: r.u16(),
                name_and_type_index: r.u16(),
            },
            19 => ConstantPoolEntry::Module { name_index: r.u16() },
            20 => ConstantPoolEntry::Package { name_index: r.u16() },
            other => return Err(format!("Unknown constant pool tag: {other} at index {i}")),
        };
        entries.push(entry);
        i += 1;
    }
    Ok(ConstantPool::new(entries))
}

fn parse_field(r: &mut Reader, cp: &ConstantPool) -> Result<FieldInfo, String> {
    let access_flags = r.u16();
    let name_index = r.u16();
    let descriptor_index = r.u16();
    let attr_count = r.u16();
    let attributes = (0..attr_count)
        .map(|_| parse_attribute(r, cp))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FieldInfo { access_flags, name_index, descriptor_index, attributes })
}

fn parse_method(r: &mut Reader, cp: &ConstantPool) -> Result<MethodInfo, String> {
    let access_flags = r.u16();
    let name_index = r.u16();
    let descriptor_index = r.u16();
    let attr_count = r.u16();
    let attributes = (0..attr_count)
        .map(|_| parse_attribute(r, cp))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MethodInfo { access_flags, name_index, descriptor_index, attributes })
}

fn parse_attribute(r: &mut Reader, cp: &ConstantPool) -> Result<Attribute, String> {
    let name_index = r.u16();
    let length = r.u32() as usize;
    let name = cp.utf8(name_index);

    match name {
        "Code" => {
            let max_stack = r.u16();
            let max_locals = r.u16();
            let code_length = r.u32() as usize;
            let code = r.bytes(code_length);
            let exception_count = r.u16();
            let exception_table = (0..exception_count)
                .map(|_| ExceptionTableEntry {
                    start_pc: r.u16(),
                    end_pc: r.u16(),
                    handler_pc: r.u16(),
                    catch_type: r.u16(),
                })
                .collect();
            let attr_count = r.u16();
            let attributes = (0..attr_count)
                .map(|_| parse_attribute(r, cp))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Attribute::Code(CodeAttribute {
                max_stack,
                max_locals,
                code,
                exception_table,
                attributes,
            }))
        }
        "ConstantValue" => Ok(Attribute::ConstantValue { constantvalue_index: r.u16() }),
        "BootstrapMethods" => {
            let num_bootstrap_methods = r.u16();
            let methods = (0..num_bootstrap_methods)
                .map(|_| {
                    let bootstrap_method_ref = r.u16();
                    let num_bootstrap_arguments = r.u16();
                    let bootstrap_arguments = (0..num_bootstrap_arguments).map(|_| r.u16()).collect();
                    BootstrapMethod { bootstrap_method_ref, bootstrap_arguments }
                })
                .collect();
            Ok(Attribute::BootstrapMethods(methods))
        }
        "Exceptions" => {
            let number_of_exceptions = r.u16();
            let exception_index_table = (0..number_of_exceptions).map(|_| r.u16()).collect();
            Ok(Attribute::Exceptions { exception_index_table })
        }
        "LineNumberTable" => {
            let len = r.u16();
            let entries = (0..len)
                .map(|_| LineNumberEntry { start_pc: r.u16(), line_number: r.u16() })
                .collect();
            Ok(Attribute::LineNumberTable(entries))
        }
        "LocalVariableTable" => {
            let len = r.u16();
            let entries = (0..len)
                .map(|_| LocalVariableEntry {
                    start_pc: r.u16(),
                    length: r.u16(),
                    name_index: r.u16(),
                    descriptor_index: r.u16(),
                    index: r.u16(),
                })
                .collect();
            Ok(Attribute::LocalVariableTable(entries))
        }
        "SourceFile" => Ok(Attribute::SourceFile { sourcefile_index: r.u16() }),
        "Signature" => Ok(Attribute::Signature { signature_index: r.u16() }),
        "InnerClasses" => {
            let number_of_classes = r.u16();
            let classes = (0..number_of_classes)
                .map(|_| InnerClassEntry {
                    inner_class_info_index: r.u16(),
                    outer_class_info_index: r.u16(),
                    inner_name_index: r.u16(),
                    inner_class_access_flags: r.u16(),
                })
                .collect();
            Ok(Attribute::InnerClasses(classes))
        }
        "EnclosingMethod" => Ok(Attribute::EnclosingMethod {
            class_index: r.u16(),
            method_index: r.u16(),
        }),
        "NestHost" => Ok(Attribute::NestHost { host_class_index: r.u16() }),
        "NestMembers" => {
            let number_of_classes = r.u16();
            let classes = (0..number_of_classes).map(|_| r.u16()).collect();
            Ok(Attribute::NestMembers { classes })
        }
        "PermittedSubclasses" => {
            let number_of_classes = r.u16();
            let classes = (0..number_of_classes).map(|_| r.u16()).collect();
            Ok(Attribute::PermittedSubclasses { classes })
        }
        "Record" => {
            let component_count = r.u16();
            let components = (0..component_count)
                .map(|_| {
                    let name_index = r.u16();
                    let descriptor_index = r.u16();
                    let attr_count = r.u16();
                    let attributes = (0..attr_count)
                        .map(|_| parse_attribute(r, cp))
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok::<RecordComponentInfo, String>(RecordComponentInfo { name_index, descriptor_index, attributes })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Attribute::Record { components })
        }
        _ => {
            let data = r.bytes(length);
            Ok(Attribute::Unknown { name: name.to_owned(), data })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_magic() {
        let data = [0xDE, 0xAD, 0xBE, 0xEF];
        assert!(parse(&data).is_err());
    }
}
