use bevy::prelude::*;

use crate::data::OrbitAnimation;

pub fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    mut animation: ResMut<OrbitAnimation>,
    mut notifications: ResMut<crate::data::Notifications>,
) {
    if keys.just_pressed(KeyCode::Space) {
        animation.paused = !animation.paused;
        notifications.show(if animation.paused {
            "Animation en pause"
        } else {
            "Animation reprise"
        });
    }
}
