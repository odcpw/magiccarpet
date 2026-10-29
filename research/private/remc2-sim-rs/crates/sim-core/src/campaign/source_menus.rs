//! Source player-menu transitions, independent of presentation and arena access.

use sim_session::AbandonAction;

/// `MouseAndKeysEvents_17A00` sends action 20 with these menu destinations.
/// Confirmation admission stays with the local player input slot owner.
pub(crate) fn destination(menu: u8, action: AbandonAction) -> Option<u8> {
    match (menu, action) {
        (0, AbandonAction::ToggleGameplayMap) => Some(6),
        (6 | 7, AbandonAction::ToggleGameplayMap) => Some(0),
        (0, AbandonAction::OpenSpellMenu) => Some(5),
        (6 | 7, AbandonAction::OpenSpellMenu) => Some(8),
        (5, AbandonAction::CloseSpellMenu) => Some(0),
        (8, AbandonAction::CloseSpellMenu) => Some(6),
        (6 | 7, AbandonAction::ShowPlayerInformation) => Some(7),
        (7, AbandonAction::HidePlayerInformation) => Some(6),
        (0 | 5..=8, AbandonAction::CloseGameplayMenus) => Some(0),
        (0, AbandonAction::OpenAbandonConfirmation) => Some(13),
        (6 | 7, AbandonAction::OpenAbandonConfirmation) => Some(14),
        (13, AbandonAction::CancelAbandonConfirmation) => Some(0),
        (14, AbandonAction::CancelAbandonConfirmation) => Some(6),
        _ => None,
    }
}

pub(crate) const fn map_open(menu: u8) -> bool {
    matches!(menu, 6..=8 | 14)
}

pub(crate) const fn spells_open(menu: u8) -> bool {
    matches!(menu, 5 | 8)
}

/// `SetMenuCursorPosition_52E90` hides text without changing its speech phase.
pub(crate) const fn hides_objective_textbox(menu: u8) -> bool {
    matches!(menu, 3 | 7 | 9..=14)
}

/// `MouseAndKeysEvents_17A00` calls `ProcessKeyboardPresses_17190` only
/// from these source menu branches. The keyboard handler itself requires
/// an empty retained local player action slot before reading held O.
pub(crate) const fn admits_keyboard_input(menu: u8, player_action: Option<i8>) -> bool {
    matches!(menu, 0 | 1 | 4 | 5 | 6 | 7 | 8) && matches!(player_action, Some(0))
}

/// `sub_1A7A0_fly_asistant` excludes spells and chat, not the map.
pub(crate) const fn suppresses_flight_assistance(menu: u8) -> bool {
    matches!(menu, 3 | 5 | 8)
}

#[cfg(test)]
mod tests {
    use super::admits_keyboard_input;

    #[test]
    fn objective_keyboard_branches_require_an_empty_source_action_slot() {
        for menu in [0, 1, 4, 5, 6, 7, 8] {
            assert!(admits_keyboard_input(menu, Some(0)), "menu {menu}");
            assert!(!admits_keyboard_input(menu, Some(27)), "menu {menu}");
            assert!(!admits_keyboard_input(menu, None), "menu {menu}");
        }
        for menu in [2, 3, 9, 10, 11, 12, 13, 14, 15] {
            assert!(!admits_keyboard_input(menu, Some(0)), "menu {menu}");
        }
    }
}
