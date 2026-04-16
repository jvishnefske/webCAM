//! Type system for the DAG -- covers CBOR major types and MLIR builtin scalars.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;

/// Reference to a struct definition in the TypeRegistry.
pub type TypeId = u16;

/// Data types for DAG values, covering all CBOR major types and MLIR builtin scalars.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DagType {
    // -- MLIR builtin scalars (CBOR major 0/1/7) --
    #[default]
    F64,
    F32,
    F16,
    /// MLIR `i1`, CBOR simple true/false.
    Bool,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    /// MLIR `index`, target-dependent width.
    Index,

    // -- CBOR compound types (major 2/3/4/5) --
    /// CBOR major 2: byte string.
    Bytes,
    /// CBOR major 3: text string.
    Text,
    /// CBOR major 4: homogeneous array.
    Array(Box<DagType>),
    /// CBOR major 5: key-value map.
    Map(Box<DagType>, Box<DagType>),

    // -- Structured types --
    /// Named product type, refers to TypeRegistry.
    Struct(TypeId),
    /// Nullable wrapper (CBOR null).
    Optional(Box<DagType>),
}

impl DagType {
    /// True for scalar numeric and boolean types (not Bytes/Text/Array/Map/Struct/Optional).
    pub fn is_scalar(&self) -> bool {
        matches!(
            self,
            DagType::F64
                | DagType::F32
                | DagType::F16
                | DagType::Bool
                | DagType::I8
                | DagType::I16
                | DagType::I32
                | DagType::I64
                | DagType::U8
                | DagType::U16
                | DagType::U32
                | DagType::U64
                | DagType::Index
        )
    }

    /// True for F16, F32, F64.
    pub fn is_float(&self) -> bool {
        matches!(self, DagType::F16 | DagType::F32 | DagType::F64)
    }

    /// True for any integer (signed or unsigned), excluding Bool and Index.
    pub fn is_integer(&self) -> bool {
        self.is_signed_integer() || self.is_unsigned_integer()
    }

    /// True for I8, I16, I32, I64.
    pub fn is_signed_integer(&self) -> bool {
        matches!(
            self,
            DagType::I8 | DagType::I16 | DagType::I32 | DagType::I64
        )
    }

    /// True for U8, U16, U32, U64.
    pub fn is_unsigned_integer(&self) -> bool {
        matches!(
            self,
            DagType::U8 | DagType::U16 | DagType::U32 | DagType::U64
        )
    }

    /// Byte width for fixed-size types. `None` for variable-length
    /// (Bytes/Text/Array/Map/Struct/Optional).
    pub fn byte_width(&self) -> Option<usize> {
        match self {
            DagType::Bool | DagType::I8 | DagType::U8 => Some(1),
            DagType::F16 | DagType::I16 | DagType::U16 => Some(2),
            DagType::F32 | DagType::I32 | DagType::U32 => Some(4),
            DagType::F64 | DagType::I64 | DagType::U64 | DagType::Index => Some(8),
            DagType::Bytes
            | DagType::Text
            | DagType::Array(_)
            | DagType::Map(_, _)
            | DagType::Struct(_)
            | DagType::Optional(_) => None,
        }
    }

    /// MLIR type name string.
    pub fn mlir_name(&self) -> String {
        match self {
            DagType::F64 => String::from("f64"),
            DagType::F32 => String::from("f32"),
            DagType::F16 => String::from("f16"),
            DagType::Bool => String::from("i1"),
            DagType::I8 => String::from("i8"),
            DagType::I16 => String::from("i16"),
            DagType::I32 => String::from("i32"),
            DagType::I64 => String::from("i64"),
            DagType::U8 => String::from("ui8"),
            DagType::U16 => String::from("ui16"),
            DagType::U32 => String::from("ui32"),
            DagType::U64 => String::from("ui64"),
            DagType::Index => String::from("index"),
            DagType::Bytes => String::from("memref<?xi8>"),
            DagType::Text => String::from("!llvm.ptr"),
            DagType::Array(t) => format!("memref<?x{}>", t.mlir_name()),
            DagType::Map(k, v) => format!("!map<{},{}>", k.mlir_name(), v.mlir_name()),
            DagType::Struct(id) => format!("!struct<{}>", id),
            DagType::Optional(t) => format!("!optional<{}>", t.mlir_name()),
        }
    }

    /// CBOR major type number (0=uint, 1=negint, 2=bstr, 3=tstr, 4=array, 5=map,
    /// 7=float/simple).
    pub fn cbor_major_type(&self) -> u8 {
        match self {
            DagType::U8 | DagType::U16 | DagType::U32 | DagType::U64 | DagType::Index => 0,
            DagType::I8 | DagType::I16 | DagType::I32 | DagType::I64 => 0,
            DagType::Bytes => 2,
            DagType::Text => 3,
            DagType::Array(_) => 4,
            DagType::Map(_, _) | DagType::Struct(_) => 5,
            DagType::F16 | DagType::F32 | DagType::F64 | DagType::Bool | DagType::Optional(_) => 7,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeSet;

    // ---- Classification: is_scalar ----

    #[test]
    fn scalar_types_are_scalar() {
        let scalars = [
            DagType::F64,
            DagType::F32,
            DagType::F16,
            DagType::Bool,
            DagType::I8,
            DagType::I16,
            DagType::I32,
            DagType::I64,
            DagType::U8,
            DagType::U16,
            DagType::U32,
            DagType::U64,
            DagType::Index,
        ];
        for ty in &scalars {
            assert!(ty.is_scalar(), "{:?} should be scalar", ty);
        }
    }

    #[test]
    fn non_scalar_types_are_not_scalar() {
        let non_scalars: [DagType; 5] = [
            DagType::Bytes,
            DagType::Text,
            DagType::Array(Box::new(DagType::F32)),
            DagType::Map(Box::new(DagType::Text), Box::new(DagType::I32)),
            DagType::Optional(Box::new(DagType::F64)),
        ];
        for ty in &non_scalars {
            assert!(!ty.is_scalar(), "{:?} should not be scalar", ty);
        }
        assert!(!DagType::Struct(0).is_scalar());
    }

    // ---- Classification: is_float ----

    #[test]
    fn float_types() {
        assert!(DagType::F16.is_float());
        assert!(DagType::F32.is_float());
        assert!(DagType::F64.is_float());
    }

    #[test]
    fn non_float_types() {
        assert!(!DagType::Bool.is_float());
        assert!(!DagType::I32.is_float());
        assert!(!DagType::U64.is_float());
        assert!(!DagType::Index.is_float());
        assert!(!DagType::Bytes.is_float());
        assert!(!DagType::Text.is_float());
        assert!(!DagType::Array(Box::new(DagType::F32)).is_float());
    }

    // ---- Classification: is_integer ----

    #[test]
    fn integer_types() {
        let integers = [
            DagType::I8,
            DagType::I16,
            DagType::I32,
            DagType::I64,
            DagType::U8,
            DagType::U16,
            DagType::U32,
            DagType::U64,
        ];
        for ty in &integers {
            assert!(ty.is_integer(), "{:?} should be integer", ty);
        }
    }

    #[test]
    fn non_integer_types() {
        assert!(!DagType::Bool.is_integer());
        assert!(!DagType::Index.is_integer());
        assert!(!DagType::F32.is_integer());
        assert!(!DagType::F64.is_integer());
        assert!(!DagType::F16.is_integer());
        assert!(!DagType::Bytes.is_integer());
        assert!(!DagType::Text.is_integer());
    }

    // ---- Classification: is_signed_integer ----

    #[test]
    fn signed_integer_types() {
        assert!(DagType::I8.is_signed_integer());
        assert!(DagType::I16.is_signed_integer());
        assert!(DagType::I32.is_signed_integer());
        assert!(DagType::I64.is_signed_integer());
    }

    #[test]
    fn not_signed_integer_types() {
        assert!(!DagType::U8.is_signed_integer());
        assert!(!DagType::U32.is_signed_integer());
        assert!(!DagType::Bool.is_signed_integer());
        assert!(!DagType::F64.is_signed_integer());
        assert!(!DagType::Index.is_signed_integer());
    }

    // ---- Classification: is_unsigned_integer ----

    #[test]
    fn unsigned_integer_types() {
        assert!(DagType::U8.is_unsigned_integer());
        assert!(DagType::U16.is_unsigned_integer());
        assert!(DagType::U32.is_unsigned_integer());
        assert!(DagType::U64.is_unsigned_integer());
    }

    #[test]
    fn not_unsigned_integer_types() {
        assert!(!DagType::I8.is_unsigned_integer());
        assert!(!DagType::I32.is_unsigned_integer());
        assert!(!DagType::Bool.is_unsigned_integer());
        assert!(!DagType::F64.is_unsigned_integer());
        assert!(!DagType::Index.is_unsigned_integer());
    }

    // ---- byte_width ----

    #[test]
    fn byte_width_fixed_size_types() {
        assert_eq!(DagType::Bool.byte_width(), Some(1));
        assert_eq!(DagType::I8.byte_width(), Some(1));
        assert_eq!(DagType::U8.byte_width(), Some(1));
        assert_eq!(DagType::F16.byte_width(), Some(2));
        assert_eq!(DagType::I16.byte_width(), Some(2));
        assert_eq!(DagType::U16.byte_width(), Some(2));
        assert_eq!(DagType::F32.byte_width(), Some(4));
        assert_eq!(DagType::I32.byte_width(), Some(4));
        assert_eq!(DagType::U32.byte_width(), Some(4));
        assert_eq!(DagType::F64.byte_width(), Some(8));
        assert_eq!(DagType::I64.byte_width(), Some(8));
        assert_eq!(DagType::U64.byte_width(), Some(8));
        assert_eq!(DagType::Index.byte_width(), Some(8));
    }

    #[test]
    fn byte_width_variable_size_types() {
        assert_eq!(DagType::Bytes.byte_width(), None);
        assert_eq!(DagType::Text.byte_width(), None);
        assert_eq!(DagType::Array(Box::new(DagType::F32)).byte_width(), None);
        assert_eq!(
            DagType::Map(Box::new(DagType::Text), Box::new(DagType::I32)).byte_width(),
            None
        );
        assert_eq!(DagType::Struct(0).byte_width(), None);
        assert_eq!(DagType::Optional(Box::new(DagType::F64)).byte_width(), None);
    }

    // ---- mlir_name: scalar variants ----

    #[test]
    fn mlir_name_scalars() {
        assert_eq!(DagType::F64.mlir_name(), "f64");
        assert_eq!(DagType::F32.mlir_name(), "f32");
        assert_eq!(DagType::F16.mlir_name(), "f16");
        assert_eq!(DagType::Bool.mlir_name(), "i1");
        assert_eq!(DagType::I8.mlir_name(), "i8");
        assert_eq!(DagType::I16.mlir_name(), "i16");
        assert_eq!(DagType::I32.mlir_name(), "i32");
        assert_eq!(DagType::I64.mlir_name(), "i64");
        assert_eq!(DagType::U8.mlir_name(), "ui8");
        assert_eq!(DagType::U16.mlir_name(), "ui16");
        assert_eq!(DagType::U32.mlir_name(), "ui32");
        assert_eq!(DagType::U64.mlir_name(), "ui64");
        assert_eq!(DagType::Index.mlir_name(), "index");
    }

    #[test]
    fn mlir_name_bytes_and_text() {
        assert_eq!(DagType::Bytes.mlir_name(), "memref<?xi8>");
        assert_eq!(DagType::Text.mlir_name(), "!llvm.ptr");
    }

    // ---- mlir_name: compound variants ----

    #[test]
    fn mlir_name_array() {
        let ty = DagType::Array(Box::new(DagType::F32));
        assert_eq!(ty.mlir_name(), "memref<?xf32>");
    }

    #[test]
    fn mlir_name_map() {
        let ty = DagType::Map(Box::new(DagType::Text), Box::new(DagType::I32));
        assert_eq!(ty.mlir_name(), "!map<!llvm.ptr,i32>");
    }

    #[test]
    fn mlir_name_optional() {
        let ty = DagType::Optional(Box::new(DagType::F64));
        assert_eq!(ty.mlir_name(), "!optional<f64>");
    }

    #[test]
    fn mlir_name_struct() {
        let ty = DagType::Struct(0);
        assert_eq!(ty.mlir_name(), "!struct<0>");
        let ty2 = DagType::Struct(42);
        assert_eq!(ty2.mlir_name(), "!struct<42>");
    }

    // ---- cbor_major_type ----

    #[test]
    fn cbor_major_type_unsigned() {
        assert_eq!(DagType::U8.cbor_major_type(), 0);
        assert_eq!(DagType::U16.cbor_major_type(), 0);
        assert_eq!(DagType::U32.cbor_major_type(), 0);
        assert_eq!(DagType::U64.cbor_major_type(), 0);
        assert_eq!(DagType::Index.cbor_major_type(), 0);
    }

    #[test]
    fn cbor_major_type_signed() {
        assert_eq!(DagType::I8.cbor_major_type(), 0);
        assert_eq!(DagType::I16.cbor_major_type(), 0);
        assert_eq!(DagType::I32.cbor_major_type(), 0);
        assert_eq!(DagType::I64.cbor_major_type(), 0);
    }

    #[test]
    fn cbor_major_type_compound() {
        assert_eq!(DagType::Bytes.cbor_major_type(), 2);
        assert_eq!(DagType::Text.cbor_major_type(), 3);
        assert_eq!(DagType::Array(Box::new(DagType::F32)).cbor_major_type(), 4);
        assert_eq!(
            DagType::Map(Box::new(DagType::Text), Box::new(DagType::I32)).cbor_major_type(),
            5
        );
        assert_eq!(DagType::Struct(0).cbor_major_type(), 5);
    }

    #[test]
    fn cbor_major_type_simple_float() {
        assert_eq!(DagType::F16.cbor_major_type(), 7);
        assert_eq!(DagType::F32.cbor_major_type(), 7);
        assert_eq!(DagType::F64.cbor_major_type(), 7);
        assert_eq!(DagType::Bool.cbor_major_type(), 7);
        assert_eq!(
            DagType::Optional(Box::new(DagType::I32)).cbor_major_type(),
            7
        );
    }

    // ---- Default ----

    #[test]
    fn default_is_f64() {
        assert_eq!(DagType::default(), DagType::F64);
    }

    // ---- Clone, PartialEq, Eq ----

    #[test]
    fn clone_and_eq() {
        let original = DagType::Array(Box::new(DagType::Map(
            Box::new(DagType::Text),
            Box::new(DagType::I32),
        )));
        let cloned = original.clone();
        assert_eq!(original, cloned);
        assert_ne!(original, DagType::F64);
    }

    // ---- Hash ----

    #[test]
    fn hash_usable_in_set() {
        // BTreeSet tests Ord-like usage; for Hash we use a manual check.
        // DagType derives Hash, so we can put it in a HashSet.
        // Since alloc does not include HashSet, we verify Hash is implemented
        // by computing hashes directly.
        use core::hash::{Hash, Hasher};

        struct SimpleHasher(u64);
        impl Hasher for SimpleHasher {
            fn finish(&self) -> u64 {
                self.0
            }
            fn write(&mut self, bytes: &[u8]) {
                for &b in bytes {
                    self.0 = self.0.wrapping_mul(31).wrapping_add(b as u64);
                }
            }
        }

        fn hash_of(t: &DagType) -> u64 {
            let mut h = SimpleHasher(0);
            t.hash(&mut h);
            h.finish()
        }

        let h1 = hash_of(&DagType::F64);
        let h2 = hash_of(&DagType::F32);
        // Different types should (almost certainly) have different hashes.
        assert_ne!(h1, h2);

        // Same type should have same hash.
        let h3 = hash_of(&DagType::F64);
        assert_eq!(h1, h3);
    }

    #[test]
    fn dag_type_in_btree_set() {
        let mut set = BTreeSet::new();
        // BTreeSet requires Ord; DagType does not derive Ord.
        // Instead, we test via Debug string as keys to demonstrate distinct values.
        set.insert(alloc::format!("{:?}", DagType::F64));
        set.insert(alloc::format!("{:?}", DagType::F32));
        set.insert(alloc::format!("{:?}", DagType::F64));
        assert_eq!(set.len(), 2);
    }

    // ---- Nested types ----

    #[test]
    fn nested_array_of_array() {
        let ty = DagType::Array(Box::new(DagType::Array(Box::new(DagType::F32))));
        assert_eq!(ty.mlir_name(), "memref<?xmemref<?xf32>>");
        assert!(!ty.is_scalar());
        assert_eq!(ty.byte_width(), None);
        assert_eq!(ty.cbor_major_type(), 4);
    }

    #[test]
    fn nested_optional_array() {
        let ty = DagType::Optional(Box::new(DagType::Array(Box::new(DagType::I32))));
        assert_eq!(ty.mlir_name(), "!optional<memref<?xi32>>");
        assert!(!ty.is_scalar());
        assert_eq!(ty.byte_width(), None);
        assert_eq!(ty.cbor_major_type(), 7);
    }

    // ---- Serde round-trip (feature-gated) ----

    #[cfg(feature = "serde")]
    mod serde_tests {
        use super::*;

        fn roundtrip(ty: &DagType) -> DagType {
            let json = serde_json::to_string(ty).expect("serialize");
            serde_json::from_str(&json).expect("deserialize")
        }

        #[test]
        fn serde_scalar_roundtrip() {
            let types = [
                DagType::F64,
                DagType::F32,
                DagType::F16,
                DagType::Bool,
                DagType::I8,
                DagType::I16,
                DagType::I32,
                DagType::I64,
                DagType::U8,
                DagType::U16,
                DagType::U32,
                DagType::U64,
                DagType::Index,
            ];
            for ty in &types {
                assert_eq!(&roundtrip(ty), ty, "serde roundtrip failed for {:?}", ty);
            }
        }

        #[test]
        fn serde_compound_roundtrip() {
            let types = [
                DagType::Bytes,
                DagType::Text,
                DagType::Array(Box::new(DagType::F32)),
                DagType::Map(Box::new(DagType::Text), Box::new(DagType::I32)),
                DagType::Struct(42),
                DagType::Optional(Box::new(DagType::F64)),
            ];
            for ty in &types {
                assert_eq!(&roundtrip(ty), ty, "serde roundtrip failed for {:?}", ty);
            }
        }

        #[test]
        fn serde_nested_roundtrip() {
            let ty = DagType::Optional(Box::new(DagType::Array(Box::new(DagType::Map(
                Box::new(DagType::Text),
                Box::new(DagType::Struct(7)),
            )))));
            assert_eq!(roundtrip(&ty), ty);
        }
    }
}
