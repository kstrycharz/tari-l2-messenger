use tari_ootle_transaction::args;
use tari_template_lib::types::ComponentAddress;
use tari_template_test_tooling::TemplateTest;

/// Placeholder pubkey hex strings for use as "to" addresses in DM tests.
const MINOTARI_PK: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const OOTLE_PK: &str    = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn deploy(test: &mut TemplateTest, sk: &tari_template_test_tooling::crypto::RistrettoSecretKey) -> ComponentAddress {
    let template_addr = test.get_template_address("MessagingService");
    let tx = test
        .transaction()
        .call_function(template_addr, "new", args![])
        .put_last_instruction_output_on_workspace("component")
        .build_and_seal(sk);
    let result = test.execute_expect_success(tx, vec![]);
    let diff = result.finalize.result.any_accept().expect("transaction accepted");
    diff.up_iter()
        .find_map(|(id, _)| id.as_component_address())
        .expect("component address in receipt")
}

#[test]
fn test_deploy_messaging_service() {
    let mut test = TemplateTest::new(".", ["."]);
    let (_, _, ootle_sk) = test.create_funded_account();
    let _component = deploy(&mut test, &ootle_sk);
    println!("MessagingService deployed successfully");
}

#[test]
fn test_send_dm_increases_count() {
    let mut test = TemplateTest::new(".", ["."]);
    let (_, ootle_proof, ootle_sk) = test.create_funded_account();
    let component = deploy(&mut test, &ootle_sk);

    let tx = test
        .transaction()
        .call_method(component, "send_dm", args![MINOTARI_PK.to_string(), "Hello Minotari!".to_string()])
        .build_and_seal(&ootle_sk);
    test.execute_expect_success(tx, vec![ootle_proof.clone()]);

    let count: u64 = test.call_method(component, "dm_count", args![], vec![ootle_proof.clone()]);
    assert_eq!(count, 1, "Expected 1 DM");
    println!("DM sent successfully, count={count}");
}

#[test]
fn test_multiple_dms_increase_count() {
    let mut test = TemplateTest::new(".", ["."]);
    let (_, ootle_proof, ootle_sk) = test.create_funded_account();
    let component = deploy(&mut test, &ootle_sk);

    for i in 0..3u32 {
        let tx = test
            .transaction()
            .call_method(component, "send_dm", args![MINOTARI_PK.to_string(), format!("Message {i}")])
            .build_and_seal(&ootle_sk);
        test.execute_expect_success(tx, vec![ootle_proof.clone()]);
    }

    let count: u64 = test.call_method(component, "dm_count", args![], vec![ootle_proof.clone()]);
    assert_eq!(count, 3, "Expected 3 DMs");
    println!("3 DMs sent, count={count}");
}

#[test]
fn test_empty_dm_content_fails() {
    let mut test = TemplateTest::new(".", ["."]);
    let (_, ootle_proof, ootle_sk) = test.create_funded_account();
    let component = deploy(&mut test, &ootle_sk);

    let tx = test
        .transaction()
        .call_method(component, "send_dm", args![MINOTARI_PK.to_string(), "".to_string()])
        .build_and_seal(&ootle_sk);
    test.execute_expect_failure(tx, vec![ootle_proof.clone()]);
    println!("Empty DM content correctly rejected");
}

#[test]
fn test_empty_recipient_fails() {
    let mut test = TemplateTest::new(".", ["."]);
    let (_, ootle_proof, ootle_sk) = test.create_funded_account();
    let component = deploy(&mut test, &ootle_sk);

    let tx = test
        .transaction()
        .call_method(component, "send_dm", args!["".to_string(), "Hello!".to_string()])
        .build_and_seal(&ootle_sk);
    test.execute_expect_failure(tx, vec![ootle_proof.clone()]);
    println!("Empty recipient correctly rejected");
}

#[test]
fn test_create_room_and_list() {
    let mut test = TemplateTest::new(".", ["."]);
    let (_, ootle_proof, ootle_sk) = test.create_funded_account();
    let component = deploy(&mut test, &ootle_sk);

    let tx = test
        .transaction()
        .call_method(component, "create_room", args!["room-general".to_string(), "General".to_string()])
        .build_and_seal(&ootle_sk);
    test.execute_expect_success(tx, vec![ootle_proof.clone()]);

    let tx = test
        .transaction()
        .call_method(component, "create_room", args!["room-random".to_string(), "Random".to_string()])
        .build_and_seal(&ootle_sk);
    test.execute_expect_success(tx, vec![ootle_proof.clone()]);

    // list_rooms returns [id, name, creator_pk, id, name, creator_pk, ...]
    let rooms: Vec<String> = test.call_method(component, "list_rooms", args![], vec![ootle_proof.clone()]);
    assert_eq!(rooms.len(), 6, "2 rooms × 3 fields each = 6");
    assert_eq!(rooms[0], "room-general");
    assert_eq!(rooms[1], "General");
    assert_eq!(rooms[3], "room-random");
    assert_eq!(rooms[4], "Random");
    println!("Rooms listed correctly: {:?}", rooms);
}

#[test]
fn test_duplicate_room_id_fails() {
    let mut test = TemplateTest::new(".", ["."]);
    let (_, ootle_proof, ootle_sk) = test.create_funded_account();
    let component = deploy(&mut test, &ootle_sk);

    let tx = test
        .transaction()
        .call_method(component, "create_room", args!["room-abc".to_string(), "ABC".to_string()])
        .build_and_seal(&ootle_sk);
    test.execute_expect_success(tx, vec![ootle_proof.clone()]);

    let tx = test
        .transaction()
        .call_method(component, "create_room", args!["room-abc".to_string(), "ABC Duplicate".to_string()])
        .build_and_seal(&ootle_sk);
    test.execute_expect_failure(tx, vec![ootle_proof.clone()]);
    println!("Duplicate room ID correctly rejected");
}

#[test]
fn test_post_to_room_and_retrieve() {
    let mut test = TemplateTest::new(".", ["."]);
    let (_, ootle_proof, ootle_sk)       = test.create_funded_account();
    let (_, minotari_proof, minotari_sk) = test.create_funded_account();
    let component = deploy(&mut test, &ootle_sk);

    let tx = test
        .transaction()
        .call_method(component, "create_room", args!["general".to_string(), "General".to_string()])
        .build_and_seal(&ootle_sk);
    test.execute_expect_success(tx, vec![ootle_proof.clone()]);

    // Ootle posts
    let tx = test
        .transaction()
        .call_method(component, "post_to_room", args!["general".to_string(), "Hey everyone!".to_string()])
        .build_and_seal(&ootle_sk);
    test.execute_expect_success(tx, vec![ootle_proof.clone()]);

    // Minotari posts
    let tx = test
        .transaction()
        .call_method(component, "post_to_room", args!["general".to_string(), "Hi Ootle!".to_string()])
        .build_and_seal(&minotari_sk);
    test.execute_expect_success(tx, vec![minotari_proof.clone()]);

    // get_room_messages returns [from_pk, content, from_pk, content, ...]
    let msgs: Vec<String> = test.call_method(
        component,
        "get_room_messages",
        args!["general".to_string()],
        vec![ootle_proof.clone()],
    );
    assert_eq!(msgs.len(), 4, "2 messages × 2 fields each = 4");
    assert_eq!(msgs[1], "Hey everyone!");
    assert_eq!(msgs[3], "Hi Ootle!");
    println!("Room messages correct: {:?}", msgs);
}

#[test]
fn test_post_to_nonexistent_room_fails() {
    let mut test = TemplateTest::new(".", ["."]);
    let (_, ootle_proof, ootle_sk) = test.create_funded_account();
    let component = deploy(&mut test, &ootle_sk);

    let tx = test
        .transaction()
        .call_method(component, "post_to_room", args!["no-such-room".to_string(), "hello".to_string()])
        .build_and_seal(&ootle_sk);
    test.execute_expect_failure(tx, vec![ootle_proof.clone()]);
    println!("Post to nonexistent room correctly rejected");
}

#[test]
fn test_room_message_count() {
    let mut test = TemplateTest::new(".", ["."]);
    let (_, ootle_proof, ootle_sk) = test.create_funded_account();
    let component = deploy(&mut test, &ootle_sk);

    let count: u64 = test.call_method(component, "room_message_count", args![], vec![ootle_proof.clone()]);
    assert_eq!(count, 0, "Should start at 0");

    let tx = test
        .transaction()
        .call_method(component, "create_room", args!["main".to_string(), "Main".to_string()])
        .build_and_seal(&ootle_sk);
    test.execute_expect_success(tx, vec![ootle_proof.clone()]);

    let tx = test
        .transaction()
        .call_method(component, "post_to_room", args!["main".to_string(), "First post!".to_string()])
        .build_and_seal(&ootle_sk);
    test.execute_expect_success(tx, vec![ootle_proof.clone()]);

    let count: u64 = test.call_method(component, "room_message_count", args![], vec![ootle_proof.clone()]);
    assert_eq!(count, 1, "Should be 1 after one post");
    println!("Room message count correct: {count}");
}

#[test]
fn test_dm_conversation_both_directions() {
    let mut test = TemplateTest::new(".", ["."]);
    let (_, ootle_proof, ootle_sk)       = test.create_funded_account();
    let (_, minotari_proof, minotari_sk) = test.create_funded_account();
    let component = deploy(&mut test, &ootle_sk);

    // Ootle → MINOTARI_PK
    let tx = test
        .transaction()
        .call_method(component, "send_dm", args![MINOTARI_PK.to_string(), "Hello from Ootle".to_string()])
        .build_and_seal(&ootle_sk);
    test.execute_expect_success(tx, vec![ootle_proof.clone()]);

    // Minotari → OOTLE_PK
    let tx = test
        .transaction()
        .call_method(component, "send_dm", args![OOTLE_PK.to_string(), "Hello from Minotari".to_string()])
        .build_and_seal(&minotari_sk);
    test.execute_expect_success(tx, vec![minotari_proof.clone()]);

    let count: u64 = test.call_method(component, "dm_count", args![], vec![ootle_proof.clone()]);
    assert_eq!(count, 2, "Expected 2 total DMs");
    println!("Two-way DM: {count} messages");
}
