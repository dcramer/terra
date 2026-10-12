use crate::{Catalogue, Item, ItemId, ItemKind, Refused};

/// A width and height in cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Size {
    pub w: u8,
    pub h: u8,
}

impl Size {
    pub const ONE: Size = Size { w: 1, h: 1 };
}

/// A cell in a grid, from the top left.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Cell {
    pub x: u8,
    pub y: u8,
}

/// An item and where its top-left cell is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    pub item: Item,
    pub at: Cell,
    pub size: Size,
    /// If it is a container for a stack, what is in it (D118).
    pub stack: Option<Stack>,
}

/// A stack in a container: of what kind, how many at most, and how many now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stack {
    pub kind: ItemKind,
    pub most: u32,
    pub held: u32,
}

impl Placed {
    fn overlaps(&self, at: Cell, size: Size) -> bool {
        let (ax, ay) = (self.at.x as u16, self.at.y as u16);
        let (bx, by) = (at.x as u16, at.y as u16);
        ax < bx + size.w as u16
            && bx < ax + self.size.w as u16
            && ay < by + size.h as u16
            && by < ay + self.size.h as u16
    }
}

/// A container: a backpack, a locker, a bag left where someone went down. Each item fills
/// a rectangle of cells, and no two overlap. Items are not turned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    size: Size,
    placed: Vec<Placed>,
}

impl Grid {
    pub fn new(w: u8, h: u8) -> Self {
        Self {
            size: Size { w, h },
            placed: Vec::new(),
        }
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn iter(&self) -> impl Iterator<Item = &Placed> {
        self.placed.iter()
    }

    pub fn len(&self) -> usize {
        self.placed.len()
    }

    pub fn is_empty(&self) -> bool {
        self.placed.is_empty()
    }

    pub fn get(&self, id: ItemId) -> Option<&Placed> {
        self.placed.iter().find(|placed| placed.item.id == id)
    }

    /// The item filling a cell, if any.
    pub fn at(&self, cell: Cell) -> Option<&Placed> {
        self.placed
            .iter()
            .find(|placed| placed.overlaps(cell, Size::ONE))
    }

    /// How many items of a kind it holds, loose and in containers' stacks.
    pub fn count(&self, kind: ItemKind) -> u32 {
        let loose = self
            .placed
            .iter()
            .filter(|placed| placed.item.kind == kind)
            .count() as u32;
        loose + self.stacked(kind)
    }

    /// How many of a kind are in containers' stacks.
    pub fn stacked(&self, kind: ItemKind) -> u32 {
        self.placed
            .iter()
            .filter_map(|placed| placed.stack)
            .filter(|stack| stack.kind == kind)
            .map(|stack| stack.held)
            .sum()
    }

    /// Puts up to `count` of a kind into containers with room for them. Returns how many
    /// went in; the rest are for the caller to place loose, or not at all.
    pub fn stow(&mut self, kind: ItemKind, count: u32) -> u32 {
        let mut left = count;
        for stack in self
            .placed
            .iter_mut()
            .filter_map(|placed| placed.stack.as_mut())
        {
            if stack.kind == kind && left > 0 {
                let room = stack.most - stack.held;
                let put = room.min(left);
                stack.held += put;
                left -= put;
            }
        }
        count - left
    }

    /// Sets how much a container holds, as when a saved one is put back. Refused if it is
    /// not a container, or that is more than it holds.
    pub fn set_held(&mut self, id: ItemId, held: u32) -> Result<(), Refused> {
        let placed = self
            .placed
            .iter_mut()
            .find(|placed| placed.item.id == id)
            .ok_or(Refused::NotFound)?;
        match placed.stack.as_mut() {
            Some(stack) if held <= stack.most => {
                stack.held = held;
                Ok(())
            }
            _ => Err(Refused::NoRoom),
        }
    }

    /// Whether something of this size could go here, ignoring one item (the one being
    /// moved, if it is moving within this grid).
    pub fn fits(&self, at: Cell, size: Size, ignoring: Option<ItemId>) -> bool {
        at.x as u16 + size.w as u16 <= self.size.w as u16
            && at.y as u16 + size.h as u16 <= self.size.h as u16
            && !self
                .placed
                .iter()
                .any(|placed| Some(placed.item.id) != ignoring && placed.overlaps(at, size))
    }

    /// The first place something of this size fits, scanning rows from the top left.
    pub fn space_for(&self, size: Size) -> Option<Cell> {
        (0..self.size.h)
            .flat_map(|y| (0..self.size.w).map(move |x| Cell { x, y }))
            .find(|&cell| self.fits(cell, size, None))
    }

    /// The first place something of this size fits without reaching past `bounds`, as when
    /// the grid is about to shrink to that size.
    pub fn space_within(&self, size: Size, bounds: Size) -> Option<Cell> {
        let (w, h) = (bounds.w.min(self.size.w), bounds.h.min(self.size.h));
        (0..h)
            .flat_map(|y| (0..w).map(move |x| Cell { x, y }))
            .find(|&cell| {
                cell.x as u16 + size.w as u16 <= w as u16
                    && cell.y as u16 + size.h as u16 <= h as u16
                    && self.fits(cell, size, None)
            })
    }

    /// Put an item in, at a cell or wherever it first fits.
    pub fn insert(
        &mut self,
        catalogue: &Catalogue,
        item: Item,
        at: Option<Cell>,
    ) -> Result<Cell, Refused> {
        let size = catalogue.get(item.kind)?.size;
        let at = match at {
            Some(cell) if self.fits(cell, size, None) => cell,
            Some(_) => return Err(Refused::NoRoom),
            None => self.space_for(size).ok_or(Refused::NoRoom)?,
        };
        let def = catalogue.get(item.kind)?;
        let stack = def.holds.map(|(kind, most)| Stack {
            kind,
            most,
            held: 0,
        });
        self.placed.push(Placed {
            item,
            at,
            size: def.size,
            stack,
        });
        Ok(at)
    }

    /// Wears an item by so much use (D99). Returns whether that broke it. An item of a
    /// kind that does not wear is unchanged.
    pub fn wear(
        &mut self,
        catalogue: &Catalogue,
        id: ItemId,
        amount: u16,
    ) -> Result<bool, Refused> {
        let placed = self
            .placed
            .iter_mut()
            .find(|placed| placed.item.id == id)
            .ok_or(Refused::NotFound)?;
        let def = catalogue.get(placed.item.kind)?;
        Ok(placed.item.wear_by(def, amount))
    }

    /// Sets how worn an item is: to 0 when it is mended, or as a saved game says.
    pub fn set_wear(&mut self, id: ItemId, wear: u16) -> Result<(), Refused> {
        let placed = self
            .placed
            .iter_mut()
            .find(|placed| placed.item.id == id)
            .ok_or(Refused::NotFound)?;
        placed.item.wear = wear;
        Ok(())
    }

    /// Take an item out.
    pub fn remove(&mut self, id: ItemId) -> Result<Item, Refused> {
        let index = self
            .placed
            .iter()
            .position(|placed| placed.item.id == id)
            .ok_or(Refused::NotFound)?;
        Ok(self.placed.swap_remove(index).item)
    }

    /// Move an item to another cell of this grid.
    pub fn shift(&mut self, id: ItemId, to: Cell) -> Result<(), Refused> {
        let index = self
            .placed
            .iter()
            .position(|placed| placed.item.id == id)
            .ok_or(Refused::NotFound)?;
        if !self.fits(to, self.placed[index].size, Some(id)) {
            return Err(Refused::NoRoom);
        }
        self.placed[index].at = to;
        Ok(())
    }

    /// Take so many of each kind, all or nothing: what building spends (D55). Stacks in
    /// containers are drawn on first, then loose items, which are returned.
    pub fn take(&mut self, cost: &[(ItemKind, u32)]) -> Result<Vec<Item>, Refused> {
        if cost.iter().any(|&(kind, count)| self.count(kind) < count) {
            return Err(Refused::NotEnough);
        }
        let mut taken = Vec::new();
        for &(kind, count) in cost {
            let mut left = count;
            for stack in self
                .placed
                .iter_mut()
                .filter_map(|placed| placed.stack.as_mut())
            {
                if stack.kind == kind {
                    let drawn = stack.held.min(left);
                    stack.held -= drawn;
                    left -= drawn;
                }
            }
            for _ in 0..left {
                let id = self
                    .placed
                    .iter()
                    .rev()
                    .find(|placed| placed.item.kind == kind)
                    .map(|placed| placed.item.id)
                    .ok_or(Refused::NotEnough)?;
                taken.push(self.remove(id)?);
            }
        }
        Ok(taken)
    }

    /// Change the grid's size, as when a bigger or smaller backpack is worn. Refused if
    /// anything would be left outside it.
    pub fn resize(&mut self, size: Size) -> Result<(), Refused> {
        let inside = self.placed.iter().all(|placed| {
            placed.at.x as u16 + placed.size.w as u16 <= size.w as u16
                && placed.at.y as u16 + placed.size.h as u16 <= size.h as u16
        });
        if !inside {
            return Err(Refused::WouldNotFit);
        }
        self.size = size;
        Ok(())
    }
}

/// Move an item from one grid to another, at a cell or wherever it first fits.
pub fn transfer(
    catalogue: &Catalogue,
    from: &mut Grid,
    to: &mut Grid,
    id: ItemId,
    at: Option<Cell>,
) -> Result<Cell, Refused> {
    let moving = *from.get(id).ok_or(Refused::NotFound)?;
    let cell = to.insert(catalogue, moving.item, at)?;
    // A container takes what is in it along.
    if let Some(stack) = moving.stack {
        to.set_held(moving.item.id, stack.held)?;
    }
    from.remove(id)?;
    Ok(cell)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IdSource, KindDef};

    fn setup() -> (Catalogue, ItemKind, ItemKind, IdSource) {
        let mut catalogue = Catalogue::default();
        let ore = catalogue.add(KindDef::new("Ore"));
        let tank = catalogue.add(KindDef::new("Tank").size(2, 3));
        (catalogue, ore, tank, IdSource::default())
    }

    #[test]
    fn a_container_holds_a_stack_that_counts_pays_and_travels() {
        let mut catalogue = Catalogue::default();
        let arrow = catalogue.add(KindDef::new("Arrow"));
        let quiver = catalogue.add(KindDef::new("Quiver").size(1, 2).holds(arrow, 20));
        let mut ids = IdSource::default();
        let mut grid = Grid::new(3, 3);
        let held = ids.make(quiver);
        grid.insert(&catalogue, held, None).unwrap();
        grid.insert(&catalogue, ids.make(arrow), None).unwrap();
        // Fills to what it holds, and says how many went in.
        assert_eq!(grid.stow(arrow, 25), 20);
        assert_eq!(grid.count(arrow), 21, "the stack and the one loose arrow");
        assert_eq!(grid.len(), 2, "the stack fills no cells of its own");
        // Spends the stack first, then loose ones, all or nothing.
        assert!(grid.take(&[(arrow, 22)]).is_err());
        assert_eq!(grid.count(arrow), 21);
        assert!(grid.take(&[(arrow, 20)]).unwrap().is_empty());
        assert_eq!(grid.count(arrow), 1);
        // Moved to another grid, it takes what is in it.
        grid.stow(arrow, 7);
        let mut bag = Grid::new(2, 2);
        transfer(&catalogue, &mut grid, &mut bag, held.id, None).unwrap();
        assert_eq!(bag.count(arrow), 7);
        assert_eq!(grid.count(arrow), 1);
        assert!(bag.set_held(held.id, 21).is_err());
    }

    #[test]
    fn items_go_in_the_first_place_they_fit() {
        let (catalogue, ore, tank, mut ids) = setup();
        let mut grid = Grid::new(4, 4);
        let first = grid.insert(&catalogue, ids.make(ore), None).unwrap();
        assert_eq!(first, Cell { x: 0, y: 0 });
        let big = grid.insert(&catalogue, ids.make(tank), None).unwrap();
        assert_eq!(big, Cell { x: 1, y: 0 });
        let next = grid.insert(&catalogue, ids.make(ore), None).unwrap();
        assert_eq!(next, Cell { x: 3, y: 0 });
        assert!(grid.at(Cell { x: 2, y: 2 }).is_some());
        assert!(grid.at(Cell { x: 0, y: 3 }).is_none());
    }

    #[test]
    fn nothing_overlaps_or_hangs_outside() {
        let (catalogue, ore, tank, mut ids) = setup();
        let mut grid = Grid::new(3, 3);
        let tank = ids.make(tank);
        assert_eq!(
            grid.insert(&catalogue, tank, Some(Cell { x: 2, y: 0 })),
            Err(Refused::NoRoom)
        );
        grid.insert(&catalogue, tank, Some(Cell { x: 0, y: 0 }))
            .unwrap();
        assert_eq!(
            grid.insert(&catalogue, ids.make(ore), Some(Cell { x: 1, y: 2 })),
            Err(Refused::NoRoom)
        );
        assert!(grid.shift(tank.id, Cell { x: 1, y: 0 }).is_ok());
        assert_eq!(
            grid.shift(tank.id, Cell { x: 2, y: 0 }),
            Err(Refused::NoRoom)
        );
    }

    #[test]
    fn a_full_grid_refuses() {
        let (catalogue, ore, _, mut ids) = setup();
        let mut grid = Grid::new(2, 1);
        grid.insert(&catalogue, ids.make(ore), None).unwrap();
        grid.insert(&catalogue, ids.make(ore), None).unwrap();
        assert_eq!(
            grid.insert(&catalogue, ids.make(ore), None),
            Err(Refused::NoRoom)
        );
    }

    #[test]
    fn taking_is_all_or_nothing() {
        let (catalogue, ore, tank, mut ids) = setup();
        let mut grid = Grid::new(4, 4);
        for _ in 0..3 {
            grid.insert(&catalogue, ids.make(ore), None).unwrap();
        }
        let before = grid.clone();
        assert_eq!(grid.take(&[(ore, 2), (tank, 1)]), Err(Refused::NotEnough));
        assert_eq!(grid, before);
        assert_eq!(grid.take(&[(ore, 2)]).unwrap().len(), 2);
        assert_eq!(grid.count(ore), 1);
    }

    #[test]
    fn transfer_moves_or_changes_nothing() {
        let (catalogue, _, tank, mut ids) = setup();
        let mut from = Grid::new(4, 4);
        let mut to = Grid::new(1, 1);
        let tank = ids.make(tank);
        from.insert(&catalogue, tank, None).unwrap();
        assert_eq!(
            transfer(&catalogue, &mut from, &mut to, tank.id, None),
            Err(Refused::NoRoom)
        );
        assert_eq!(from.len(), 1);
        let mut to = Grid::new(2, 3);
        transfer(&catalogue, &mut from, &mut to, tank.id, None).unwrap();
        assert!(from.is_empty());
        assert_eq!(to.get(tank.id).unwrap().item, tank);
    }

    #[test]
    fn shrinking_is_refused_while_things_would_fall_out() {
        let (catalogue, ore, _, mut ids) = setup();
        let mut grid = Grid::new(4, 4);
        grid.insert(&catalogue, ids.make(ore), Some(Cell { x: 3, y: 3 }))
            .unwrap();
        assert_eq!(grid.resize(Size { w: 2, h: 2 }), Err(Refused::WouldNotFit));
        assert_eq!(grid.size(), Size { w: 4, h: 4 });
        assert!(grid.resize(Size { w: 6, h: 6 }).is_ok());
    }
}
