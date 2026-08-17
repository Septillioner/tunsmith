use anyhow::{Context, Result};
use rand::rngs::OsRng;
use rcgen::string::Ia5String;
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa,
    Issuer, KeyPair, KeyUsagePurpose, SanType, PKCS_RSA_SHA256,
};
use rsa::pkcs8::{EncodePrivateKey, LineEnding};
use rsa::RsaPrivateKey;
use std::net::IpAddr;
use std::str::FromStr;
use time::{Duration, OffsetDateTime};

use crate::constants::{CA_KEY_BITS, COUNTRY_CODE_LEN, DAYS_PER_YEAR, LEAF_KEY_BITS};
use crate::project::{ca_cert_path, ca_key_path, write_public_file, write_secret_file};

pub struct CaMaterial {
    pub cert_pem: String,
    pub key_pem: String,
}

pub struct LeafMaterial {
    pub cert_pem: String,
    pub key_pem: String,
}

pub struct Subject {
    pub common_name: String,
    pub organization: Option<String>,
    pub country: Option<String>,
}

pub fn generate_root_ca(subject: &Subject, validity_years: u32) -> Result<CaMaterial> {
    let key_pem = generate_rsa_pkcs8_pem(CA_KEY_BITS)?;
    let key_pair = KeyPair::from_pem_and_sign_algo(&key_pem, &PKCS_RSA_SHA256)
        .context("failed to parse generated CA key")?;

    let mut params = CertificateParams::new(Vec::<String>::new())?;
    params.distinguished_name = distinguished_name(subject)?;
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    set_validity(&mut params, validity_years);

    let cert = params.self_signed(&key_pair)?;
    Ok(CaMaterial {
        cert_pem: cert.pem(),
        key_pem,
    })
}

pub fn issue_server_cert(
    ca: &CaMaterial,
    organization: &str,
    host: &str,
    validity_years: u32,
) -> Result<LeafMaterial> {
    let key_pem = generate_rsa_pkcs8_pem(LEAF_KEY_BITS)?;
    let key_pair = KeyPair::from_pem_and_sign_algo(&key_pem, &PKCS_RSA_SHA256)
        .context("failed to parse generated server key")?;

    let mut params = CertificateParams::new(Vec::<String>::new())?;
    params.distinguished_name = distinguished_name(&Subject {
        common_name: crate::constants::SERVER_CERT_CN.to_string(),
        organization: Some(organization.to_string()),
        country: None,
    })?;
    params.is_ca = IsCa::NoCa;
    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    params.subject_alt_names = vec![host_to_san(host)?];
    set_validity(&mut params, validity_years);

    let issuer_key = KeyPair::from_pem_and_sign_algo(&ca.key_pem, &PKCS_RSA_SHA256)
        .context("failed to parse CA key")?;
    let issuer =
        Issuer::from_ca_cert_pem(&ca.cert_pem, issuer_key).context("failed to load CA issuer")?;
    let cert = params.signed_by(&key_pair, &issuer)?;

    Ok(LeafMaterial {
        cert_pem: cert.pem(),
        key_pem,
    })
}

pub fn issue_client_cert(
    ca: &CaMaterial,
    client_name: &str,
    validity_years: u32,
) -> Result<LeafMaterial> {
    let key_pem = generate_rsa_pkcs8_pem(LEAF_KEY_BITS)?;
    let key_pair = KeyPair::from_pem_and_sign_algo(&key_pem, &PKCS_RSA_SHA256)
        .context("failed to parse generated client key")?;

    let mut params = CertificateParams::new(Vec::<String>::new())?;
    params.distinguished_name = distinguished_name(&Subject {
        common_name: client_name.to_string(),
        organization: None,
        country: None,
    })?;
    params.is_ca = IsCa::NoCa;
    params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
    set_validity(&mut params, validity_years);

    let issuer_key = KeyPair::from_pem_and_sign_algo(&ca.key_pem, &PKCS_RSA_SHA256)
        .context("failed to parse CA key")?;
    let issuer =
        Issuer::from_ca_cert_pem(&ca.cert_pem, issuer_key).context("failed to load CA issuer")?;
    let cert = params.signed_by(&key_pair, &issuer)?;

    Ok(LeafMaterial {
        cert_pem: cert.pem(),
        key_pem,
    })
}

pub fn load_ca() -> Result<CaMaterial> {
    let cert_pem =
        std::fs::read_to_string(ca_cert_path()).context("failed to read CA certificate")?;
    let key_pem =
        std::fs::read_to_string(ca_key_path()).context("failed to read CA private key")?;
    Ok(CaMaterial { cert_pem, key_pem })
}

pub fn save_ca(ca: &CaMaterial) -> Result<()> {
    write_public_file(&ca_cert_path(), &ca.cert_pem)?;
    write_secret_file(&ca_key_path(), &ca.key_pem)?;
    Ok(())
}

fn generate_rsa_pkcs8_pem(bits: usize) -> Result<String> {
    let private_key = RsaPrivateKey::new(&mut OsRng, bits)
        .with_context(|| format!("failed to generate {bits}-bit RSA key"))?;
    let pem = private_key
        .to_pkcs8_pem(LineEnding::LF)
        .context("failed to encode RSA key as PKCS#8 PEM")?;
    Ok(pem.to_string())
}

fn distinguished_name(subject: &Subject) -> Result<DistinguishedName> {
    let mut name = DistinguishedName::new();
    name.push(DnType::CommonName, &subject.common_name);
    if let Some(org) = &subject.organization {
        name.push(DnType::OrganizationName, org);
    }
    if let Some(country) = &subject.country {
        if country.len() != COUNTRY_CODE_LEN || !country.chars().all(|c| c.is_ascii_alphabetic()) {
            anyhow::bail!("country code must be two ASCII letters");
        }
        name.push(DnType::CountryName, country.to_uppercase());
    }
    Ok(name)
}

fn set_validity(params: &mut CertificateParams, validity_years: u32) {
    let not_before = OffsetDateTime::now_utc();
    let days = i64::from(validity_years) * DAYS_PER_YEAR;
    params.not_before = not_before;
    params.not_after = not_before + Duration::days(days);
}

fn host_to_san(host: &str) -> Result<SanType> {
    if let Ok(ip) = IpAddr::from_str(host) {
        return Ok(SanType::IpAddress(ip));
    }
    Ok(SanType::DnsName(Ia5String::try_from(host.to_string())?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{DEFAULT_CA_VALIDITY_YEARS, DEFAULT_DNS_PRIMARY, LEAF_KEY_BITS};

    const PKCS8_PEM_BEGIN: &str = "-----BEGIN PRIVATE KEY-----";
    const PKCS8_PEM_END: &str = "-----END PRIVATE KEY-----";
    const CERT_PEM_BEGIN: &str = "-----BEGIN CERTIFICATE-----";

    #[test]
    fn generate_leaf_size_rsa_key_is_pkcs8_pem() {
        let pem = generate_rsa_pkcs8_pem(LEAF_KEY_BITS).unwrap();
        assert!(pem.starts_with(PKCS8_PEM_BEGIN));
        assert!(pem.contains(PKCS8_PEM_END));
        assert!(!pem.contains('\r'));
    }

    #[test]
    fn distinguished_name_rejects_invalid_country_codes() {
        let valid = Subject {
            common_name: "ca".to_string(),
            organization: None,
            country: Some("US".to_string()),
        };
        assert!(distinguished_name(&valid).is_ok());

        let too_long = Subject {
            common_name: "ca".to_string(),
            organization: None,
            country: Some("USA".to_string()),
        };
        assert!(distinguished_name(&too_long).is_err());

        let non_alpha = Subject {
            common_name: "ca".to_string(),
            organization: None,
            country: Some("U1".to_string()),
        };
        assert!(distinguished_name(&non_alpha).is_err());
    }

    #[test]
    fn host_to_san_distinguishes_ip_from_dns() {
        match host_to_san(DEFAULT_DNS_PRIMARY).unwrap() {
            SanType::IpAddress(ip) => {
                assert_eq!(ip, IpAddr::from_str(DEFAULT_DNS_PRIMARY).unwrap());
            }
            other => panic!("expected IP SAN, got {other:?}"),
        }

        match host_to_san("vpn.example.com").unwrap() {
            SanType::DnsName(name) => assert_eq!(name.as_str(), "vpn.example.com"),
            other => panic!("expected DNS SAN, got {other:?}"),
        }
    }

    // RSA-4096 CA generation is too slow for the default suite (~1 min).
    #[test]
    #[ignore]
    fn generate_root_ca_emits_certificate_and_key_pem() {
        let ca = generate_root_ca(
            &Subject {
                common_name: "Tunsmith Test CA".to_string(),
                organization: Some("Tunsmith".to_string()),
                country: Some("US".to_string()),
            },
            DEFAULT_CA_VALIDITY_YEARS,
        )
        .unwrap();
        assert!(ca.cert_pem.contains(CERT_PEM_BEGIN));
        assert!(ca.key_pem.contains(PKCS8_PEM_BEGIN));
    }
}
