//! Building laid out (D407): where a base's pieces go by its kits' rules ([`layout`]), how a
//! door or shutters hang in the hole they are fitted into ([`fitting`]), and what holds a
//! walker up and what stops them ([`walk`]), as boxes for a character controller
//! ([`solids`]). Over the rules (`terra_build`), under the drawing (`terra_build_mesh`); what a
//! server needs of building past the rules. Pure and repeatable: no engine, `libm` for
//! trigonometry (D30).

pub mod fitting;
pub mod layout;
pub mod solids;
pub mod walk;

#[cfg(test)]
mod tests;
