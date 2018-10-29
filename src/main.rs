#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate gumdrop;
extern crate cert_machine;
extern crate openssl;

mod arg_parser;
mod config_parser;
mod kubernetes_certs;

use kubernetes_certs::gen_main_ca_cert;
use cert_machine::Bundle;
use kubernetes_certs::gen_ca_cert;
use kubernetes_certs::write_bundle_to_file;
use arg_parser::{CommandOptions, Command};
use config_parser::Config;
use gumdrop::Options;
// use openssl::pkey::PKey;
use std::process::exit;
use std::fs;

struct CA {
    main_ca: Box<Bundle>,
    etcd_ca: Box<Bundle>,
    front_ca: Box<Bundle>,
}

impl CA {
    fn read_from_fs(dir: &str) -> CA {
        CA {
            main_ca: Bundle::read_from_fs(&dir, "ca").unwrap(),
            etcd_ca: Bundle::read_from_fs(&dir, "etcd/etcd-ca").unwrap(),
            front_ca: Bundle::read_from_fs(&dir, "front-proxy-ca").unwrap(),
        }
    }
}

fn create_ca(config: &Config, out_dir: &str) -> Result<CA, &'static str> {
    println!("Create CA: ROOT");
    let main_ca = match gen_main_ca_cert(&config) {
        Ok(bundle) => {
            write_bundle_to_file(&bundle, &out_dir, "ca");
            bundle
        },
        Err(error) => return Err(error),
    };

    println!("Create CA: etcd");
    let etcd_ca = match gen_ca_cert("etcd", Some(&main_ca)) {
        Ok(bundle) => {
            write_bundle_to_file(&bundle,  &out_dir,"etcd/etcd-ca");
            bundle
        },
        Err(error) => return Err(error),

    };

    println!("Create CA: front proxy");
    let front_ca = match gen_ca_cert("front-proxy-ca", Some(&main_ca)) {
        Ok(bundle) => {
            write_bundle_to_file(&bundle, &out_dir, "front-proxy-ca");
            bundle
        },
        Err(error) => return Err(error),

    };
    Ok(CA {
        main_ca,
        etcd_ca,
        front_ca,
    })
}

fn main() {
    let opts = CommandOptions::parse_args_default_or_exit();

    println!("{:#?}",opts);

    let config = Config::new("config.toml");
    let out_dir = "certs".to_owned();

    println!("Creating output dirs.");
    let etcd_dir = format!("{}/etcd", out_dir);
    match fs::create_dir_all(etcd_dir) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Error when creating dir: {:#?}", e);
            exit(1);
        }
    }

    match opts.command {
        Some(Command::New(_)) => {
            let ca = match create_ca(&config, &out_dir) {
                Ok(ca) => ca,
                Err(err) => {
                    panic!("Error when creating certificate authority: {}", err);

                },
            };

            for instance in config.worker.iter() {
                kubernetes_certs::gen_kubelet_cert(&instance, &ca.main_ca.private_key(), &ca.main_ca.cert);
            }

            for instance in config.etcd_server.iter() {
                kubernetes_certs::gen_etcd_cert(&instance, &ca.etcd_ca.private_key(), &ca.etcd_ca.cert);
            }

            kubernetes_certs::kube_certs(&ca.main_ca.private_key(), &ca.main_ca.cert, &config, &out_dir, &ca.front_ca);
        },
        Some(Command::InitCa(_)) => {
            match create_ca(&config, &out_dir) {
                Ok(ca) => ca,
                Err(err) => {
                    panic!("Error when creating certificate authority: {}", err);

                },
            };
        },
        Some(Command::GenCerts(_)) => {
            let ca = CA::read_from_fs("certs");

            for instance in config.worker.iter() {
                kubernetes_certs::gen_kubelet_cert(&instance, &ca.main_ca.private_key(), &ca.main_ca.cert);
            }

            for instance in config.etcd_server.iter() {
                kubernetes_certs::gen_etcd_cert(&instance, &ca.etcd_ca.private_key(), &ca.etcd_ca.cert);
            }

            kubernetes_certs::kube_certs(&ca.main_ca.private_key(), &ca.main_ca.cert, &config, &out_dir, &ca.front_ca);
        },
        None => (),
    }

    println!("Creating output dirs.");
    let etcd_dir = format!("{}/etcd", out_dir);
    match fs::create_dir_all(etcd_dir) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Error when creating dir: {:#?}", e);
            exit(1);
        }
    }
}
