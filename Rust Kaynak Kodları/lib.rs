use jni::objects::{JClass, JObject, JString};
use jni::JNIEnv;
use once_cell::sync::OnceCell;
use std::sync::Mutex;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

mod behaviour;
mod chat;
mod contacts;
mod p2p;
mod transport;

use p2p::Command;

// Global sender to communicate with the P2P task
static SENDER: OnceCell<Mutex<mpsc::Sender<Command>>> = OnceCell::new();
static RUNTIME: OnceCell<Runtime> = OnceCell::new();
static JAVA_VM: OnceCell<jni::JavaVM> = OnceCell::new();
static NATIVE_LIB_OBJ: OnceCell<Mutex<jni::objects::GlobalRef>> = OnceCell::new();
static LOCAL_PEER_ID: OnceCell<Mutex<String>> = OnceCell::new();
static LISTEN_ADDRESSES: OnceCell<Mutex<Vec<String>>> = OnceCell::new();

#[no_mangle]
pub extern "system" fn Java_com_example_thecommunication_NativeLib_startNode(
    mut env: JNIEnv,
    obj: JObject,
    seed: JString,
    identity_path: JString,
) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    log::info!("========== startNode CALLED ==========");

    // Store JavaVM for callbacks
    let jvm = env.get_java_vm().expect("Failed to get JavaVM");
    JAVA_VM.set(jvm).ok();

    // Store global reference to the NativeLib object for callbacks
    let global_ref = env
        .new_global_ref(obj)
        .expect("Failed to create global ref");
    NATIVE_LIB_OBJ.set(Mutex::new(global_ref)).ok();

    let seed_str: String = env
        .get_string(&seed)
        .expect("Couldn't get java string!")
        .into();

    let identity_path_str: String = env
        .get_string(&identity_path)
        .expect("Couldn't get identity path!")
        .into();

    log::info!(
        "Starting P2P Node with identity path: {}",
        identity_path_str
    );

    if SENDER.get().is_some() {
        log::warn!("Node already started - SENDER exists");
        return;
    }

    let (tx, rx) = mpsc::channel(32);

    // Initialize global sender
    SENDER
        .set(Mutex::new(tx.clone()))
        .expect("Failed to set sender");

    log::info!("SENDER initialized, starting background thread...");

    // Start Tokio runtime in a separate thread
    std::thread::spawn(move || {
        log::info!("Background thread started, creating runtime...");
        let rt = Runtime::new().unwrap();

        // Parse seed if provided
        let seed_byte = if !seed_str.is_empty() {
            Some(seed_str.as_bytes()[0])
        } else {
            None
        };

        log::info!("Running P2P node...");
        rt.block_on(async {
            if let Err(e) = p2p::run_p2p_node(rx, seed_byte, identity_path_str).await {
                log::error!("P2P Node failed: {:?}", e);
            }
        });
        log::info!("P2P node loop ended");
    });

    log::info!("========== startNode COMPLETED ==========");
}

// Callback function to notify Java about incoming messages
pub fn notify_message_received(sender_id: String, message: String) {
    log::info!(
        "notify_message_received called: {} - {}",
        sender_id,
        message
    );
    if let Some(jvm) = JAVA_VM.get() {
        if let Ok(mut env) = jvm.attach_current_thread() {
            if let Some(obj_mutex) = NATIVE_LIB_OBJ.get() {
                if let Ok(obj_guard) = obj_mutex.lock() {
                    let obj = obj_guard.as_obj();

                    // Convert Rust strings to Java strings
                    let j_sender_id = env.new_string(&sender_id).unwrap();
                    let j_message = env.new_string(&message).unwrap();

                    // Call Java method: onMessageReceived(String, String)
                    let result = env.call_method(
                        obj,
                        "onMessageReceived",
                        "(Ljava/lang/String;Ljava/lang/String;)V",
                        &[(&j_sender_id).into(), (&j_message).into()],
                    );

                    if let Err(e) = result {
                        log::error!("Failed to call Java callback: {:?}", e);
                    } else {
                        log::info!("Java callback called successfully");
                    }
                }
            }
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_example_thecommunication_NativeLib_dialPeer(
    mut env: JNIEnv,
    _class: JClass,
    addr: JString,
) {
    let addr_str: String = env
        .get_string(&addr)
        .expect("Couldn't get java string!")
        .into();

    log::info!("dialPeer called with: {}", addr_str);

    if let Ok(multiaddr) = addr_str.parse() {
        log::info!("Parsed multiaddr successfully, sending command...");
        send_command(Command::Dial(multiaddr));
        log::info!("Dial command sent");
    } else {
        log::error!("Invalid address: {}", addr_str);
    }
}

#[no_mangle]
pub extern "system" fn Java_com_example_thecommunication_NativeLib_sendMessage(
    mut env: JNIEnv,
    _class: JClass,
    peer_id: JString,
    message: JString,
) {
    let peer_str: String = env
        .get_string(&peer_id)
        .expect("Couldn't get java string!")
        .into();
    let msg_str: String = env
        .get_string(&message)
        .expect("Couldn't get java string!")
        .into();

    log::info!("sendMessage called: {} -> {}", peer_str, msg_str);

    if let Ok(peer) = peer_str.parse() {
        send_command(Command::SendMessage(peer, msg_str));
        log::info!("SendMessage command sent");
    } else {
        log::error!("Invalid peer ID: {}", peer_str);
    }
}

#[no_mangle]
pub extern "system" fn Java_com_example_thecommunication_NativeLib_saveContact(
    mut env: JNIEnv,
    _class: JClass,
    name: JString,
    addr: JString,
) {
    let name_str: String = env
        .get_string(&name)
        .expect("Couldn't get java string!")
        .into();
    let addr_str: String = env
        .get_string(&addr)
        .expect("Couldn't get java string!")
        .into();

    log::info!("saveContact called: {} -> {}", name_str, addr_str);
    send_command(Command::SaveContact(name_str, addr_str));
}

#[no_mangle]
pub extern "system" fn Java_com_example_thecommunication_NativeLib_connectContact(
    mut env: JNIEnv,
    _class: JClass,
    name: JString,
) {
    let name_str: String = env
        .get_string(&name)
        .expect("Couldn't get java string!")
        .into();

    log::info!("connectContact called: {}", name_str);
    send_command(Command::ConnectContact(name_str));
}

#[no_mangle]
pub extern "system" fn Java_com_example_thecommunication_NativeLib_getMyPeerId(
    _env: JNIEnv,
    _class: JClass,
) -> jni::sys::jstring {
    if let Some(peer_id_mutex) = LOCAL_PEER_ID.get() {
        if let Ok(peer_id) = peer_id_mutex.lock() {
            let output = _env
                .new_string(&*peer_id)
                .expect("Couldn't create java string!");
            return output.into_raw();
        }
    }
    _env.new_string("Unknown")
        .expect("Couldn't create java string!")
        .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_com_example_thecommunication_NativeLib_getListenAddresses(
    mut env: JNIEnv,
    _class: JClass,
) -> jni::sys::jobjectArray {
    if let Some(addrs_mutex) = LISTEN_ADDRESSES.get() {
        if let Ok(addrs) = addrs_mutex.lock() {
            let string_class = env.find_class("java/lang/String").unwrap();
            let output = env
                .new_object_array(
                    addrs.len() as i32,
                    string_class,
                    env.new_string("").unwrap(),
                )
                .unwrap();

            for (i, addr) in addrs.iter().enumerate() {
                let jstr = env.new_string(addr).unwrap();
                env.set_object_array_element(&output, i as i32, jstr)
                    .unwrap();
            }
            return output.into_raw();
        }
    }

    // Return empty array if no addresses
    let string_class = env.find_class("java/lang/String").unwrap();
    let output = env
        .new_object_array(0, string_class, env.new_string("").unwrap())
        .unwrap();
    output.into_raw()
}

fn send_command(cmd: Command) {
    log::info!("send_command called");
    if let Some(mutex) = SENDER.get() {
        log::info!("SENDER exists, acquiring lock...");
        if let Ok(tx) = mutex.lock() {
            log::info!("Lock acquired, cloning sender...");
            let tx_clone = tx.clone();
            std::thread::spawn(move || {
                log::info!("Spawned thread, sending command...");
                if let Err(e) = tx_clone.blocking_send(cmd) {
                    log::error!("Failed to send command: {}", e);
                } else {
                    log::info!("Command sent successfully!");
                }
            });
        } else {
            log::error!("Failed to lock SENDER mutex");
        }
    } else {
        log::error!("P2P Node not started - SENDER is None");
    }
}
