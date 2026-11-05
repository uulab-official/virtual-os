//! MAC address helpers.

use uuid::Uuid;

/// Generate a random, locally administered, unicast MAC address.
pub fn generate_mac() -> String {
    let bytes = Uuid::new_v4().into_bytes();
    // Clear the multicast bit and set the locally-administered bit.
    let first = (bytes[0] & 0xfc) | 0x02;
    format!(
        "{first:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        bytes[1], bytes[2], bytes[3], bytes[4], bytes[5]
    )
}

/// Parse and canonicalize a colon- or dash-separated MAC address.
///
/// Rejects multicast addresses because Virtualization.framework requires a
/// unicast address for virtual NICs.
pub fn normalize_mac(value: &str) -> Result<String, &'static str> {
    let cleaned = value.replace('-', ":");
    let parts: Vec<&str> = cleaned.split(':').collect();
    if parts.len() != 6 {
        return Err("expected six octets");
    }
    let mut octets = [0u8; 6];
    for (index, part) in parts.iter().enumerate() {
        if part.len() != 2 {
            return Err("each octet must have two hex digits");
        }
        octets[index] = u8::from_str_radix(part, 16).map_err(|_| "invalid hex octet")?;
    }
    if octets[0] & 0x01 != 0 {
        return Err("multicast MAC addresses are not allowed");
    }
    Ok(format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        octets[0], octets[1], octets[2], octets[3], octets[4], octets[5]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_macs_are_local_unicast() {
        for _ in 0..64 {
            let mac = generate_mac();
            assert_eq!(normalize_mac(&mac).unwrap(), mac);
            assert_eq!(mac.len(), 17);
            let first = u8::from_str_radix(&mac[0..2], 16).unwrap();
            assert_eq!(first & 0x01, 0);
            assert_ne!(first & 0x02, 0);
        }
    }

    #[test]
    fn accepts_dash_and_uppercase() {
        assert_eq!(
            normalize_mac("02-AA-bb-01-02-03").unwrap(),
            "02:aa:bb:01:02:03"
        );
    }

    #[test]
    fn rejects_invalid_macs() {
        for mac in [
            "",
            "02:00:00:00:00",
            "02:00:00:00:00:00:00",
            "zz:00:00:00:00:00",
            "01:00:00:00:00:00",
        ] {
            assert!(normalize_mac(mac).is_err(), "{mac:?}");
        }
    }
}
