#[macro_use]
extern crate log;
#[macro_use]
extern crate may;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate nrsc;
#[macro_use]
extern crate lazy_static;

extern crate chrono;
extern crate env_logger;
extern crate hashbrown;
extern crate may_signal;
extern crate num_cpus;
extern crate rand;
extern crate rcu_cell;
extern crate nrsc_object_base;
extern crate nrsc_wallet_base;

mod timer;
mod witness;

use nrsc::error::Result;
use nrsc::kv_store;
use nrsc::network;
use nrsc::wallet_info::MY_WALLET;

fn log_init() {
    // TODO: need to implement async logs
    let log_lvl = nrsc::config::get_log_level();
    let mut builder = env_logger::Builder::from_default_env();

    builder.filter_module("pagecache", log::LevelFilter::Error);
    builder.filter(None, log_lvl).init();

    info!("log init done!");
}

fn init() -> Result<()> {
    // init default coroutine settings
    let stack_size = if cfg!(debug_assertions) {
        0x4000
    } else {
        0x2000
    };

    let workers = nrsc::config::get_worker_thread_num();
    may::config()
        .set_stack_size(stack_size)
        .set_io_workers(num_cpus::get_physical())
        .set_workers(workers);;

    log_init();
    nrsc::config::show_config();

    kv_store::KV_STORE.rebuild_from_kv()?;

    Ok(())
}

fn start_ws_server() {
    use network::hub::WSS;
    use network::WsServer;

    if let Some(addr) = nrsc::config::get_listen_address() {
        let _server = WsServer::start(&addr, |c| {
            t!(WSS.add_p2p_conn(c, true));
        })
        .ok();
        println!("Websocket server running on ws://{}", addr);
    }
}

fn connect_to_remote() -> Result<()> {
    let peers = nrsc::config::get_remote_hub_url();
    let mut is_no_connect = false;
    for peer in peers {
        match nrsc::network::hub::create_outbound_conn(&peer) {
            Ok(_) => is_no_connect = true,
            Err(e) => error!(" fail to connected: {}, err={}", peer, e),
        }
    }
    if !is_no_connect {
        bail!("can not connect to any peers");
    }

    Ok(())
}

fn network_cleanup() {
    network::hub::WSS.close_all();
}

// the hub server logic that run in coroutine context
fn run_hub_server() -> Result<()> {
    start_ws_server();
    connect_to_remote()?;
    timer::start_global_timers();
    Ok(())
}

fn main() -> Result<()> {
    init()?;
    run_hub_server()?;

    if !nrsc::my_witness::MY_WITNESSES.contains(&MY_WALLET._00_address) {
        bail!("address {} is not witness");
    }

    // wait user input a ctrl_c to exit
    may_signal::ctrl_c().recv().unwrap();

    kv_store::KV_STORE.finish()?;

    network_cleanup();
    info!("bye from main!\n\n");
    Ok(())
}
