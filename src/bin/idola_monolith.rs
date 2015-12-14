extern crate idola;
extern crate psomsg;
#[macro_use] extern crate log;
extern crate env_logger;
extern crate rustc_serialize;
extern crate docopt;
extern crate toml;

use std::sync::Arc;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::thread;
use std::fs::File;
use std::io::Read;

use std::sync::mpsc::Sender;
use std::sync::mpsc::channel;

use docopt::Docopt;

use idola::db::sqlite::Sqlite;
use idola::db::Pool;
use idola::bb::read_key_table;

const USAGE: &'static str = "
IDOLA Phantasy Star Online 'Monolith' Server
Provides a patch, data, login, and character server all in one.

Usage:
  idola_monolith [options]
  idola_monolith (-h | --help)
  idola_monolith --version

Options:
  -h, --help          Show this message.
  --version           Show the version.
  --config=<cfg>      Set the Monolith config path. Defaults to 'monolith_config.toml'
";


#[derive(Clone, Debug, RustcDecodable)]
struct Args {
    flag_config: Option<String>,
    flag_version: bool
}

#[derive(Debug, RustcDecodable)]
struct Config {
    motd_template: String,
    bind_address: Option<String>,
    bb_keytable_path: Option<String>
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum MonolithMsg {
    Up(MonolithComponent),
    DownErr(MonolithComponent, String),
    DownGraceful(MonolithComponent)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MonolithComponent {
    Patch,
    Data,
    Login,
    Character
}

fn patch_server(channel: Sender<MonolithMsg>, motd_template: String, bind_address: String) {
    use idola::patch::*;
    use std::str::FromStr;
    use std::error::Error;
    let bind_ipv4addr = Ipv4Addr::from_str("127.0.0.1").unwrap();
    let data_servers = vec![SocketAddrV4::new(bind_ipv4addr, 11001)];
    let server = PatchServer::new_bb(motd_template.clone(), bind_address.clone(), &data_servers);
    channel.send(MonolithMsg::Up(MonolithComponent::Patch)).unwrap();
    channel.send(match server.run() {
        Err(e) => MonolithMsg::DownErr(MonolithComponent::Patch, e.description().to_string()),
        Ok(_) => MonolithMsg::DownGraceful(MonolithComponent::Patch)
    }).unwrap();
}

fn data_server(channel: Sender<MonolithMsg>) {
    use idola::patch::*;
    let data_server = DataServer::new_bb("127.0.0.1:11001");
    channel.send(MonolithMsg::Up(MonolithComponent::Data)).unwrap();
    channel.send(match data_server.run() {
        Err(e) => MonolithMsg::DownErr(MonolithComponent::Data, e.to_string()),
        Ok(_) => MonolithMsg::DownGraceful(MonolithComponent::Data)
    }).unwrap();
}

fn login_server(channel: Sender<MonolithMsg>, key_table: Arc<Vec<u32>>, db_pool: Arc<Pool>) {
    use std::net::TcpListener;
    use std::thread;

    channel.send(MonolithMsg::Up(MonolithComponent::Login)).unwrap();

    let char_server_ip = Ipv4Addr::new(127, 0, 0, 1);
    let char_server_port = 12001;

    let tcp_listener = TcpListener::bind("127.0.0.1:12000").unwrap();
    for s in tcp_listener.incoming() {
        match s {
            Ok(s) => {
                let kt_clone = key_table.clone();
                let db_clone = db_pool.clone();
                thread::spawn(move|| {
                    use idola::login::bb::{Context, run_login};
                    run_login(Context::new(s, kt_clone, db_clone, None), char_server_ip, char_server_port);
                });
            },
            Err(e) => error!("error, quitting: {}", e)
        }
    }

    channel.send(MonolithMsg::DownGraceful(MonolithComponent::Login)).unwrap();
}

fn char_server(channel: Sender<MonolithMsg>, key_table: Arc<Vec<u32>>, db_pool: Arc<Pool>, param_chunks: Arc<(psomsg::bb::Message, Vec<psomsg::bb::Message>)>) {

    channel.send(MonolithMsg::Up(MonolithComponent::Character)).unwrap();

    let tcp_listener = TcpListener::bind("127.0.0.1:12001").unwrap();
    for s in tcp_listener.incoming() {
        match s {
            Ok(s) => {
                let kt_clone = key_table.clone();
                let db_clone = db_pool.clone();
                let pc_clone = param_chunks.clone();
                thread::spawn(move|| {
                    use idola::login::bb::{Context, run_character};
                    run_character(Context::new(s, kt_clone, db_clone, Some(pc_clone)));
                });
            },
            Err(e) => error!("closing character server: {:?}", e)
        }
    }

    channel.send(MonolithMsg::DownGraceful(MonolithComponent::Character)).unwrap();
}

fn read_config(path: &str) -> Config {
    use std::fs::File;
    use std::io::Read;
    use rustc_serialize::Decodable;
    let mut config_contents = String::new();
    File::open(path).unwrap().read_to_string(&mut config_contents).unwrap();
    let config;
    {
        let mut parser = toml::Parser::new(&config_contents);
        let fields = match parser.parse() {
            None => panic!("Parsing config failed: {:?}", parser.errors),
            Some(s) => match s.get("config") {
                None => panic!("Config did not have a [config] block"),
                Some(f) => f.clone()
            }
        };
        config = Config::decode(&mut toml::Decoder::new(fields)).unwrap();
    }
    config
}

fn main() {
    use std::thread;

    if let Err(_) = std::env::var("RUST_LOG") {
        std::env::set_var("RUST_LOG", "DEBUG");
        std::env::set_var("RUST_BACKTRACE", "1");
    }
    env_logger::init().unwrap();

    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.decode())
        .unwrap_or_else(|e| e.exit());

    if args.flag_version {
        println!("idola_monolith 0.1.0");
        return
    }

    let config = read_config(&args.flag_config.clone().unwrap_or("monolith_config.toml".to_string()));
    let motd_template_clone = config.motd_template.clone();

    // Load the BB key table
    let key_table = Arc::new(read_key_table(&mut File::open(config.bb_keytable_path.unwrap_or("data/crypto/bb_table.bin".to_string())).unwrap()).unwrap());

    // Set up the DB pool
    let pool = Arc::new(Pool::new(1, &mut Sqlite::new("test.db", true).unwrap()).unwrap());

    // Load the parameter files
    let param_chunks = Arc::new(idola::login::paramfiles::load_paramfiles_msgs().unwrap());

    let (tx, rx) = channel();
    let tx_c = tx.clone();
    thread::spawn(move|| patch_server(tx_c, motd_template_clone, "127.0.0.1:11000".to_string()));
    let tx_c = tx.clone();
    thread::spawn(move|| data_server(tx_c));
    let tx_c = tx.clone();
    let kt_clone = key_table.clone();
    let pool_clone = pool.clone();
    thread::spawn(move|| login_server(tx_c, kt_clone, pool_clone));
    let tx_c = tx.clone();
    let kt_clone = key_table.clone();
    let pool_clone = pool.clone();
    thread::spawn(move|| char_server(tx_c, kt_clone, pool_clone, param_chunks));

    let mut patch_status = true;
    let mut data_status = true;
    let mut login_status = true;
    let mut char_status = true;

    for m in rx.iter() {
        use MonolithComponent::*;
        match m {
            MonolithMsg::DownErr(a, s) => {
                match a {
                    Patch => patch_status = false,
                    Data => data_status = false,
                    Login => login_status = false,
                    Character => char_status = false
                }
                error!("Down by error {:?}: {:?}", a, s);
            },
            MonolithMsg::DownGraceful(a) => {
                match a {
                    Patch => patch_status = false,
                    Data => data_status = false,
                    Login => login_status = false,
                    Character => char_status = false
                }
                info!("{:?} down gracefully", a)
            },
            _ => ()
        }

        if !patch_status && !login_status && !data_status && !char_status {
            break
        }
    }
}
