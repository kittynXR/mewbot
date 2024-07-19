use twitch_irc::message::PrivmsgMessage;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum UserRole {
    Viewer,
    Subscriber,
    VIP,
    Moderator,
    Broadcaster,
}

pub fn get_user_role(msg: &PrivmsgMessage) -> UserRole {
    if msg.badges.iter().any(|badge| badge.name == "broadcaster") {
        UserRole::Broadcaster
    } else if msg.badges.iter().any(|badge| badge.name == "moderator") {
        UserRole::Moderator
    } else if msg.badges.iter().any(|badge| badge.name == "vip") {
        UserRole::VIP
    } else if msg.badges.iter().any(|badge| badge.name == "subscriber") {
        UserRole::Subscriber
    } else {
        UserRole::Viewer
    }
}