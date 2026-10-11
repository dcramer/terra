use crate::{ItemKind, Refused, Size};

/// A kind of equipment slot: backpack, suit, tank and so on. Which slots exist is data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GearSlot(pub u8);

/// What a kind of item is. Adding an item to the game is adding one of these.
#[derive(Clone, Debug, PartialEq)]
pub struct KindDef {
    pub name: String,
    /// The cells it fills in a grid.
    pub size: Size,
    /// The equipment slot it is worn in, if it is gear.
    pub gear: Option<GearSlot>,
    /// Worn, it makes the backpack this size (D65).
    pub backpack: Option<Size>,
    /// A container for a stack of one kind, and how many it holds, as a quiver holds
    /// arrows (D118). What it holds does not fill cells of its own.
    pub holds: Option<(ItemKind, u32)>,
}

impl KindDef {
    /// A plain item of one cell.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            size: Size::ONE,
            gear: None,
            backpack: None,
            holds: None,
        }
    }

    pub fn size(mut self, w: u8, h: u8) -> Self {
        self.size = Size { w, h };
        self
    }

    pub fn gear(mut self, slot: GearSlot) -> Self {
        self.gear = Some(slot);
        self
    }

    pub fn backpack(mut self, w: u8, h: u8) -> Self {
        self.backpack = Some(Size { w, h });
        self
    }

    pub fn holds(mut self, kind: ItemKind, most: u32) -> Self {
        self.holds = Some((kind, most));
        self
    }
}

/// Every kind of item.
#[derive(Clone, Debug, Default)]
pub struct Catalogue {
    kinds: Vec<KindDef>,
}

impl Catalogue {
    pub fn add(&mut self, def: KindDef) -> ItemKind {
        self.kinds.push(def);
        ItemKind((self.kinds.len() - 1) as u16)
    }

    pub fn get(&self, kind: ItemKind) -> Result<&KindDef, Refused> {
        self.kinds.get(kind.0 as usize).ok_or(Refused::UnknownKind)
    }

    pub fn find(&self, name: &str) -> Option<ItemKind> {
        self.kinds
            .iter()
            .position(|def| def.name == name)
            .map(|index| ItemKind(index as u16))
    }

    pub fn iter(&self) -> impl Iterator<Item = (ItemKind, &KindDef)> {
        self.kinds
            .iter()
            .enumerate()
            .map(|(index, def)| (ItemKind(index as u16), def))
    }
}
