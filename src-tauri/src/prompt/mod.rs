mod compose;
mod resolver;
mod xml;

pub use compose::{compose_prompt, compose_prompt_with_memory, CompositionSkill};
pub use resolver::resolve_composition;
