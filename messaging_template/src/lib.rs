use tari_template_lib::prelude::*;

#[template]
mod messaging {
    use super::*;

    /// On-chain messaging service supporting direct messages and group rooms.
    /// Uses parallel Vec<String> fields to avoid #[template] macro limitations
    /// with custom struct types inside Vec.
    pub struct MessagingService {
        // ── Direct messages (parallel vecs) ─────────────────────
        /// Sender pubkey hex for each DM
        dm_from: Vec<String>,
        /// Recipient pubkey hex for each DM
        dm_to: Vec<String>,
        /// Content of each DM
        dm_content: Vec<String>,

        // ── Group room metadata (parallel vecs) ──────────────────
        /// Unique room identifier strings
        room_ids: Vec<String>,
        /// Human-readable room display names
        room_names: Vec<String>,
        /// Creator pubkey hex for each room
        room_creators: Vec<String>,

        // ── Group room messages (parallel vecs) ──────────────────
        /// Room ID for each group message
        room_msg_room: Vec<String>,
        /// Sender pubkey hex for each group message
        room_msg_from: Vec<String>,
        /// Content of each group message
        room_msg_content: Vec<String>,
    }

    impl MessagingService {
        /// Deploy a new messaging service component with the default test chat pre-seeded.
        pub fn new() -> Component<Self> {
            let creator = CallerContext::transaction_signer_public_key().to_string();
            Component::new(Self {
                dm_from: Vec::new(),
                dm_to: Vec::new(),
                dm_content: Vec::new(),
                room_ids: vec!["tari-messenger-test-chat".to_string()],
                room_names: vec!["Tari Messenger Test Chat".to_string()],
                room_creators: vec![creator],
                room_msg_room: Vec::new(),
                room_msg_from: Vec::new(),
                room_msg_content: Vec::new(),
            })
            .with_access_rules(ComponentAccessRules::allow_all())
            .create()
        }

        /// Send a direct message to `to` (recipient pubkey hex).
        /// The caller's pubkey is set on-chain as `from` — cannot be spoofed.
        pub fn send_dm(&mut self, to: String, content: String) {
            assert!(!content.is_empty(), "Content cannot be empty");
            assert!(content.len() <= 1024, "Content too long (max 1024 chars)");
            assert!(!to.is_empty(), "Recipient cannot be empty");

            let from = CallerContext::transaction_signer_public_key().to_string();

            emit_event("DmSent", metadata![
                "from" => from.clone(),
                "to" => to.clone(),
                "content" => content.clone()
            ]);

            self.dm_from.push(from);
            self.dm_to.push(to);
            self.dm_content.push(content);
        }

        /// Get DM conversation between two users.
        /// Returns a flat Vec where every 3 elements = [from_pk, to_pk, content].
        pub fn get_dm_conversation(&self, user_a: String, user_b: String) -> Vec<String> {
            let mut result = Vec::new();
            for i in 0..self.dm_from.len() {
                let f = &self.dm_from[i];
                let t = &self.dm_to[i];
                if (f == &user_a && t == &user_b) || (f == &user_b && t == &user_a) {
                    result.push(f.clone());
                    result.push(t.clone());
                    result.push(self.dm_content[i].clone());
                }
            }
            result
        }

        /// Create a new group room. Room IDs must be unique (<=64 chars).
        /// The caller is recorded as the room creator.
        pub fn create_room(&mut self, room_id: String, display_name: String) {
            assert!(!room_id.is_empty(), "Room ID cannot be empty");
            assert!(!display_name.is_empty(), "Room name cannot be empty");
            assert!(room_id.len() <= 64, "Room ID too long (max 64 chars)");
            assert!(display_name.len() <= 128, "Room name too long (max 128 chars)");
            assert!(!self.room_ids.contains(&room_id), "Room ID already exists");

            let creator = CallerContext::transaction_signer_public_key().to_string();

            emit_event("RoomCreated", metadata![
                "room_id" => room_id.clone(),
                "display_name" => display_name.clone(),
                "creator" => creator.clone()
            ]);

            self.room_ids.push(room_id);
            self.room_names.push(display_name);
            self.room_creators.push(creator);
        }

        /// Post a message to a group room. Room must exist.
        /// Caller's pubkey is recorded as the sender — cannot be spoofed.
        pub fn post_to_room(&mut self, room_id: String, content: String) {
            assert!(!content.is_empty(), "Content cannot be empty");
            assert!(content.len() <= 1024, "Content too long (max 1024 chars)");
            assert!(self.room_ids.contains(&room_id), "Room not found");

            let from = CallerContext::transaction_signer_public_key().to_string();

            emit_event("RoomMessage", metadata![
                "room_id" => room_id.clone(),
                "from" => from.clone(),
                "content" => content.clone()
            ]);

            self.room_msg_room.push(room_id);
            self.room_msg_from.push(from);
            self.room_msg_content.push(content);
        }

        /// Get all messages in a room.
        /// Returns a flat Vec where every 2 elements = [from_pk, content].
        pub fn get_room_messages(&self, room_id: String) -> Vec<String> {
            let mut result = Vec::new();
            for i in 0..self.room_msg_room.len() {
                if self.room_msg_room[i] == room_id {
                    result.push(self.room_msg_from[i].clone());
                    result.push(self.room_msg_content[i].clone());
                }
            }
            result
        }

        /// List all rooms. Returns a flat Vec where every 3 elements = [id, name, creator_pk].
        pub fn list_rooms(&self) -> Vec<String> {
            let mut result = Vec::new();
            for i in 0..self.room_ids.len() {
                result.push(self.room_ids[i].clone());
                result.push(self.room_names[i].clone());
                result.push(self.room_creators[i].clone());
            }
            result
        }

        /// Total number of direct messages stored.
        pub fn dm_count(&self) -> u64 {
            self.dm_from.len() as u64
        }

        /// Total number of room messages stored.
        pub fn room_message_count(&self) -> u64 {
            self.room_msg_room.len() as u64
        }
    }
}
