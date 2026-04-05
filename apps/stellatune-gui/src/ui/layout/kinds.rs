#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutKind {
    Stack,
    Row,
    Column,
    Align,
    SizedBox,
    Leaf,
}
