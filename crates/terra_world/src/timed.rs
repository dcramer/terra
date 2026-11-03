//! A system timed as its group's (D607, `terra_gen::stat`) where it is added, not inside
//! it: `terra_world::timed!("building", launch)`. For a system whose body cannot take the
//! `stat_scope!` line (a function already over the size limit, D396, which may not grow)
//! and that nothing orders itself by (`.after(launch)` names the function's system, which
//! the wrapped one is not). Every other system opens `terra_gen::stat_scope!` as its first
//! line, the same timer.

pub use bevy::ecs::system::IntoAdapterSystem;
use bevy::ecs::system::{Adapt, RunSystemError, System, SystemIn, SystemInput};
pub use terra_gen::stat::Stat;

/// Runs a system inside its group's scope.
pub struct Timed(pub &'static Stat);

impl<S: System> Adapt<S> for Timed {
    type In = S::In;
    type Out = S::Out;

    fn adapt(
        &mut self,
        input: <Self::In as SystemInput>::Inner<'_>,
        run_system: impl FnOnce(SystemIn<'_, S>) -> Result<S::Out, RunSystemError>,
    ) -> Result<Self::Out, RunSystemError> {
        let _scope = self.0.scope();
        run_system(input)
    }
}

/// The system given, timed as the named group's: `timed!("flora", follow_climate)`.
#[macro_export]
macro_rules! timed {
    ($group:literal, $system:expr) => {{
        static STAT: $crate::timed::Stat = $crate::timed::Stat::new($group);
        $crate::timed::IntoAdapterSystem::new($crate::timed::Timed(&STAT), $system)
    }};
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    fn slow() {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    #[test]
    fn a_system_added_timed_is_timed_as_its_group() {
        let mut app = App::new();
        app.add_systems(Update, crate::timed!("timed-test", slow).run_if(|| true));
        app.update();
        app.update();
        let taken = terra_gen::stat::take();
        let group = taken.iter().find(|t| t.group == "timed-test").unwrap();
        assert_eq!(group.scopes, 2, "{taken:?}");
        assert!(group.ns >= 10_000_000, "{taken:?}");
    }
}
