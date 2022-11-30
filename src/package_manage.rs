use std::fmt;
use std::{
    fs::{self, read_dir},
    io::Write,
};

use dirs;
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json;

type PackageResult<T> = Result<T, PackageError>;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum PackageError {
    Reason(String),
}

impl std::fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            PackageError::Reason(ref s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Package {
    name: String,
    version: String,
    description: String,
    requires: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct GithubFolder {
    name: String,
    path: String,
    sha: String,
    size: u64,
    url: String,
    html_url: String,
    git_url: String,
    download_url: String,
    #[serde(rename = "type")]
    type_: String,
    _links: GithubFolderLinks,
}

#[derive(Debug, Deserialize, Serialize)]
struct GithubFolderLinks {
    #[serde(rename = "self")]
    self_: String,
    git: String,
    html: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct GithubFolderResponse {
    name: Vec<GithubFolder>,
}

const BASE_URL: &str = "https://raw.githubusercontent.com/lillycat332/pscm-mods/main/";
#[allow(dead_code)]
const BASE_API_URL: &str = "https://api.github.com/repos/lillycat332/pscm-mods/contents/";
const INSTALL_VERSION: &str = "0.1.0";
const BASE_DIR: &str = ".pscm-rs";
const INSTALL_LOC: &str = "pkgs";

pub(crate) fn install_package_git_interface(package: String) -> PackageResult<()> {
    println!(
        "\x1b[32;32mpscm install version {}\x1b[0m ✨",
        INSTALL_VERSION
    );
    let pkg = get_pkg_manifest(&package)?;
    println!("Installing package: {}", pkg.clone().name);

    let x = is_pkg_installed(&package).unwrap();
    if x {
        println!("This package is already installed.");
        print!("Would you like to reinstall? (y/n) => ");
        std::io::stdout().flush().unwrap();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        if input.trim() == "y" {
            println!("Installing anyway...");
        } else {
            println!("Aborting installation.");
            return Ok(());
        }
    }

    match install_package_git(pkg.clone()) {
        Ok(_) => {
            println!("Installed package: {}", pkg.clone().name);
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn get_pkg_manifest(package: &str) -> PackageResult<Package> {
    let url = format!("{}{}/{}.pscmpkg", BASE_URL, package, package);
    let resp = get_api_instance().unwrap().get(&url).send();

    match resp {
        Ok(r) => {
            if r.status().is_success() {
                let pkg: Package = serde_json::from_str(&r.text().unwrap()).unwrap();
                Ok(pkg)
            } else {
                Err(PackageError::Reason(format!(
                    "Package {} not found",
                    package
                )))
            }
        }
        Err(e) => Err(PackageError::Reason(format!(
            "Failed to get package manifest: {}",
            e
        ))),
    }
}

fn install_package_git(package: Package) -> PackageResult<()> {
    let base_dir = dirs::home_dir().unwrap().join(BASE_DIR);
    // let install_dir = base_dir.join(INSTALL_LOC);
    let work_dir = base_dir.join("work");
    if !work_dir.exists() {
        std::fs::create_dir(&work_dir).unwrap();
    }

    let package_dir = work_dir.join(&package.name);

    if !package_dir.exists() {
        std::fs::create_dir(&package_dir).unwrap();
    }
    let package_manifest = package_dir.join(format!("{}.pscmpkg", &package.name));
    let mut file = std::fs::File::create(&package_manifest).unwrap();
    let mut buf = Vec::new();

    println!("Downloading package manifest...");

    let req = get_api_instance()
        .unwrap()
        .get(BASE_URL.to_string() + &package.name + "/" + &package.name + ".pscmpkg")
        .send()
        .unwrap()
        .copy_to(&mut buf);

    // println!(
    //     "DEBUG: {}",
    //     buf.clone()
    //         .into_iter()
    //         .map(|c| c as char)
    //         .collect::<String>()
    // );

    file.write_all(&buf).unwrap();

    match req {
        Ok(_) => {}
        Err(e) => {
            return Err(PackageError::Reason(format!(
                "failed to download package manifest: {}",
                e
            )))
        }
    };

    println!("Downloading package files...");

    get_pkgfiles(package.clone())?;

    let deps = package.requires;
    for dep in deps {
        let url = BASE_URL.to_string() + &dep + "/" + &dep + ".pscmpkg";
        println!("Fetching package manifest for dependency: {}", dep);

        let is_builtin = is_built_in_pkg(&dep);
        if is_builtin {
            println!("Dependency {} is in base libs, skipping...", dep);
            continue;
        }

        let dep_manifest = get_api_instance().unwrap().get(&url).send().unwrap();

        let dep_manifest_: Package = if dep_manifest.status().is_success() {
            serde_json::from_str(&dep_manifest.text().unwrap()).unwrap()
        } else {
            return Err(PackageError::Reason(format!(
                "failed to fetch package manifest for dependency: {}",
                dep
            )));
        };

        let should =
            is_pkg_installed(&dep).unwrap() || is_pkg_installed(&dep_manifest_.name).unwrap();

        println!("Installing dependency: {}", dep);

        if should {
            println!("Package {} already installed, looking for updates...", dep);
            let installed_pkg = get_local_pkg_manifest(&dep).unwrap();

            if check_for_update(installed_pkg).unwrap() {
                println!("Updating package {}...", dep);
                install_package_git(dep_manifest_.clone())?;
            } else {
                println!("Package {} is up to date", dep);
            }
        } else {
            install_package_git(dep_manifest_.clone())?;
        }
    }

    Ok(())
}

fn get_local_pkg_manifest(package: &str) -> PackageResult<Package> {
    let base_dir = dirs::home_dir().unwrap().join(BASE_DIR);
    let work_dir = base_dir.join("work");
    let package_dir = work_dir.join(&package);
    let package_manifest = package_dir.join(format!("{}.pscmpkg", &package));
    let pkg: Package =
        serde_json::from_str(&fs::read_to_string(&package_manifest).unwrap()).unwrap();
    Ok(pkg)
}

fn is_built_in_pkg(pkg: &str) -> bool {
    let built_in_pkgs = vec!["base"];
    built_in_pkgs.contains(&pkg)
}

fn is_pkg_installed(package: &str) -> PackageResult<bool> {
    let base_dir = dirs::home_dir().unwrap().join(BASE_DIR);
    let work_dir = base_dir.join("work");
    if !work_dir.exists() {
        std::fs::create_dir(&work_dir).unwrap();
    }

    let package_dir = work_dir.join(package);
    if !package_dir.exists() {
        Ok(false)
    } else {
        if fs::read_dir(&package_dir).unwrap().count() == 0 {
            Ok(false)
        } else {
            Ok(true)
        }
    }
}

fn get_api_instance() -> Result<reqwest::blocking::Client, reqwest::Error> {
    reqwest::blocking::Client::builder()
        .user_agent(env!("CARGO_PKG_NAME"))
        .build()
}

fn get_pkgfiles(package: Package) -> PackageResult<()> {
    let api_url = format!("{}{}", BASE_API_URL, package.name);
    //println!("DEBUG: url: {}", api_url);
    let api_client = get_api_instance().unwrap();
    let res_ = api_client.get(api_url).send().unwrap();
    if !res_.status().is_success() {
        return Err(PackageError::Reason(format!(
            "failed to fetch package files for {}: {}",
            package.name,
            res_.status()
        )));
    }
    let res: Vec<GithubFolder> = serde_json::from_str(&res_.text().unwrap()).unwrap();

    let base_dir = dirs::home_dir().unwrap().join(BASE_DIR);
    let work_dir = base_dir.join("work");
    let package_dir = base_dir.join(INSTALL_LOC).join(&package.name);

    for file in res {
        let path = BASE_URL.to_string() + &file.path;
        let pkg_dirname = package.clone().name + "-" + &package.version;
        // println!("DEBUG: dirname: {}", path);
        let dl_path = work_dir.join(".tmp").join(pkg_dirname).join(&file.path);
        // println!("DEBUG: dl_path: {}", dl_path.display());
        if !dl_path.exists() {
            std::fs::create_dir_all(&dl_path.parent().unwrap()).unwrap();
        }
        let req = get_api_instance()
            .unwrap()
            .get(path)
            .send()
            .unwrap()
            .copy_to(&mut std::fs::File::create(&dl_path).unwrap());
        match req {
            Ok(_) => {}
            Err(e) => {
                return Err(PackageError::Reason(format!(
                    "failed to download package file: {}",
                    e
                )))
            }
        };

        let files = read_dir(dl_path.parent().unwrap()).unwrap();

        files
            .map(|f| {
                let f = f.unwrap();
                let path = f.path();
                let file_name = path.file_name().unwrap().to_str().unwrap();
                let dest = package_dir.join(file_name);
                //rintln!("DEBUG: dest: {}", dest.display());
                if !dest.exists() {
                    std::fs::create_dir_all(&dest.parent().unwrap()).unwrap();
                }
                std::fs::copy(&path, &dest).unwrap();
            })
            .collect::<_>()
    }

    Ok(())
}

fn check_for_update(package: Package) -> PackageResult<bool> {
    let new_manifest = get_pkg_manifest(&package.name)?;
    if new_manifest.version != package.version {
        Ok(true)
    } else {
        Ok(false)
    }
}
