use rand;
use rand::Rng;

use crate::type_flyweight::{Config, IpServiceResult, UpdateIpResults};

mod address_as_body;

pub async fn request_ip(results: &UpdateIpResults, config: &Config) -> IpServiceResult {
    // create new ip_service result
    // preserve the last run's "current" address as this run's previous address
    let mut ip_service_result = IpServiceResult::new();
    ip_service_result.prev_address = match &results.ip_service_result.address {
        Some(address) => Some(address.clone()),
        _ => results.ip_service_result.prev_address.clone(),
    };

    // get service uri and response type or return previous results
    let (ip_service, response_type) = match get_ip_service(&results, &config) {
        Some(r) => r,
        _ => {
            ip_service_result
                .errors
                .push("failed to find ip service".to_string());
            return ip_service_result;
        }
    };

    // preserve service uri
    // set service results based on response type
    ip_service_result.service = Some(ip_service);
    ip_service_result = match response_type {
        _ => address_as_body::request_address_as_response_body(ip_service_result).await,
    };

    ip_service_result.address_changed = has_address_changed(&results, &ip_service_result);

    ip_service_result
}

fn has_address_changed(results: &UpdateIpResults, ip_service_result: &IpServiceResult) -> bool {
    match (
        &results.ip_service_result.address,
        &ip_service_result.address,
    ) {
        (Some(prev_ip), Some(curr_ip)) => prev_ip != curr_ip,
        (None, Some(_curr_ip)) => true,
        _ => false,
    }
}

fn get_ip_service(results: &UpdateIpResults, config: &Config) -> Option<(String, String)> {
    if config.ip_services.len() == 0 {
        return None;
    }

    if config.ip_services.len() == 1 {
        return Some(config.ip_services[0].clone());
    }

    // get previous service index
    let mut prev_index = None;
    if let Some(service) = &results.ip_service_result.service {
        for (index, (url, _ip_service_type)) in config.ip_services.iter().enumerate() {
            if url == service {
                prev_index = Some(index);
                break;
            };
        }
    }

    // config.ip_services might change between runs
    // possibility prev service doesn't exist
    let length = match prev_index {
        Some(_index) => config.ip_services.len() - 1,
        _ => config.ip_services.len(),
    };

    let mut rng = rand::thread_rng();
    let mut random_index = rng.gen_range(0..length);
    if let Some(index) = prev_index {
        if random_index >= index {
            random_index += 1;
        }
    }

    return Some(config.ip_services[random_index].clone());
}
