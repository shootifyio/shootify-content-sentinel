use std::{borrow::Cow, cell::RefCell};
use ic_cdk::api::{time, management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod, HttpResponse, TransformArgs,
    TransformContext,
}};
use candid::{CandidType, Decode, Encode, Nat};
use ic_stable_structures::{
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
    storable::Bound,
    DefaultMemoryImpl, StableBTreeMap, Storable,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;


type Memory = VirtualMemory<DefaultMemoryImpl>;

#[derive(Serialize, Deserialize)]
struct Context {
    bucket_start_time_index: usize,
    closing_price_index: usize,
}

#[derive(CandidType, Serialize, Deserialize, Clone)]
struct StoredImage {
    content: Vec<u8>,
    prediction_id: String,
    uploaded_by: String,
}

fn default_last_update() -> u64 {
    time()
}


#[derive(CandidType, Serialize, Deserialize, Clone)]
struct CrawlResult {
    #[serde(default)]
    prediction_id: String,
    web_entities: Vec<String>,
    full_matching_images: Vec<String>,
    pages_with_matching_images: Vec<String>,
    visually_similar_images: Vec<String>,
    #[serde(default = "default_last_update")]
    last_update: u64,
}

impl CrawlResult {
    fn set_last_update_to_now(&mut self) {
        self.last_update = time();
    }
}

#[derive(CandidType, Serialize, Deserialize, Clone, Default)]
struct StorableVecString {
    images: Vec<String>,
}

impl Storable for StorableVecString {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap_or_else(|e| {
            ic_cdk::trap(&format!("Failed to decode StorableVecString: {}", e));
        })
    }

    const BOUND: Bound = Bound::Unbounded;
}


impl Storable for CrawlResult {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap_or_else(|e| {
            ic_cdk::trap(&format!("Failed to decode CrawlResult: {}", e));
        })
    }

    const BOUND: Bound = Bound::Unbounded;
}

impl Storable for StoredImage {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap_or_else(|e| {
            ic_cdk::trap(&format!("Failed to decode StoredImage: {}", e));
        })
    }

    const BOUND: Bound = Bound::Unbounded;
}


thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static STABLE_IMAGES: RefCell<StableBTreeMap<String, StoredImage, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))),
        )
    );

    static STABLE_CRAWL_RESULTS: RefCell<StableBTreeMap<String, CrawlResult, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1))),
        )
    );

    static STABLE_SUBJECT_IMAGES: RefCell<StableBTreeMap<String, StorableVecString, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(2))),
        )
    );
}

/// Store a crawl result
fn store_crawl_result(user_id: String, image_name: String, mut result: CrawlResult) -> Result<(), String> {
    let key = format!("{}:{}", user_id, image_name);

    result.set_last_update_to_now();

    STABLE_CRAWL_RESULTS.with_borrow_mut(|results| {
        results.insert(key.clone(), result);
        ic_cdk::println!("Crawl result stored for key '{}'", key);
    });

    Ok(())
}

/// Store an image associated with a user ID
#[ic_cdk::update]
fn store_image(user_id: String, prediction_id: String, name: String, content: Vec<u8>) -> Result<(), String> {
    if user_id.is_empty() {
        return Err("User ID cannot be empty.".to_string());
    }
    if name.is_empty() {
        return Err("Image name cannot be empty.".to_string());
    }
    if content.is_empty() {
        return Err("Image content cannot be empty.".to_string());
    }

    STABLE_IMAGES.with_borrow_mut(|images| {
        if images.contains_key(&name) {
            Err(format!("An image with the name '{}' already exists.", name))
        } else {
            images.insert(
                name.clone(),
                StoredImage {
                    content,
                    prediction_id: prediction_id.clone(),
                    uploaded_by: user_id.clone(),
                },
            );
            ic_cdk::println!("Image '{}' stored successfully by user '{}'", name, user_id);
            Ok(())
        }
    })
}

#[ic_cdk::update]
fn add_image_hash(subject_id: String, image_hash: String) -> Result<(), String> {
    if subject_id.is_empty() {
        return Err("Subject ID cannot be empty.".to_string());
    }
    if image_hash.is_empty() {
        return Err("Image hash cannot be empty.".to_string());
    }

    STABLE_SUBJECT_IMAGES.with_borrow_mut(|subject_images| {
        let mut entry = subject_images.get(&subject_id).unwrap_or_default();

        // Check if the image hash already exists
        if !entry.images.contains(&image_hash) {
            entry.images.push(image_hash.clone());
            subject_images.insert(subject_id.clone(), entry);
            ic_cdk::println!("Added image_hash '{}' to subject_id '{}'", image_hash, subject_id);
        } else {
            ic_cdk::println!("Image_hash '{}' already exists for subject_id '{}'", image_hash, subject_id);
        }

        Ok(())
    })
}

#[ic_cdk::query]
fn get_image_hashes(subject_id: String) -> Result<Vec<String>, String> {
    if subject_id.is_empty() {
        return Err("Subject ID cannot be empty.".to_string());
    }

    STABLE_SUBJECT_IMAGES.with_borrow(|subject_images| {
        if let Some(entry) = subject_images.get(&subject_id) {
            Ok(entry.images.clone())
        } else {
            Err("Subject ID not found.".to_string())
        }
    })
}

/// List all crawl results for a specific user ID
#[ic_cdk::query]
fn get_crawl_results(user_id: String) -> Result<String, String> {
    ic_cdk::println!("User ID received: '{}'", user_id);

    if user_id.is_empty() {
        return Err("User ID is empty.".to_string());
    }

    let prefix = format!("{}:", user_id);

    let mut user_results = HashMap::new();
    STABLE_CRAWL_RESULTS.with_borrow(|results| {
        for (key, value) in results.iter() {
            ic_cdk::println!("Stored result key '{}'", key);
            ic_cdk::println!("Prefix '{}'", prefix);
            if key.starts_with(&prefix) {
                if let Some(image_name) = key.strip_prefix(&prefix) {
                    ic_cdk::println!("Image name '{}'", image_name);

                    user_results.insert(image_name.to_string(), value.clone());
                }
            }
        }
    });

    if user_results.is_empty() {
        Err("No crawl results found for this user.".to_string())
    } else {
        match serde_json::to_string(&user_results) {
            Ok(json_string) => {
                ic_cdk::println!("Serialized results: '{}'", json_string);
                Ok(json_string)
            }
            Err(err) => {
                ic_cdk::println!("Failed to serialize results: {}", err);
                Err("Failed to serialize results.".to_string())
            }
        }
    }
}

/// Retrieve an image by name, validating the user ID
#[ic_cdk::query]
fn get_image(user_id: String, name: String) -> Result<StoredImage, String> {
    STABLE_IMAGES.with_borrow(|images| {
        if let Some(image) = images.get(&name) {
            if image.uploaded_by == user_id {
                Ok(image)
            } else {
                Err("Access denied: You do not own this image.".to_string())
            }
        } else {
            Err("Image not found.".to_string())
        }
    })
}

/// List all images for a specific user ID
#[ic_cdk::query]
fn list_images(user_id: String) -> Vec<String> {
    STABLE_IMAGES.with_borrow(|images| {
        images
            .iter()
            .filter(|(_, image)| image.uploaded_by == user_id)
            .map(|(key, _)| key.clone())
            .collect()
    })
}

/// Delete an image by name, validating the user ID
#[ic_cdk::update]
fn delete_image(user_id: String, name: String) -> Result<(), String> {
    STABLE_IMAGES.with_borrow_mut(|images| {
        if let Some(image) = images.get(&name) {
            if image.uploaded_by == user_id {
                images.remove(&name);
                ic_cdk::println!("Image '{}' deleted successfully by user '{}'", name, user_id);
                Ok(())
            } else {
                Err("Access denied: You do not own this image.".to_string())
            }
        } else {
            Err(format!("Image '{}' not found.", name))
        }
    })
}

/// Detect an image by name, validating the user ID and storing the result
#[ic_cdk::update]
async fn detect_image(user_id: String, prediction_id: String, name: String) -> Result<String, String> {
    let stored_image = STABLE_IMAGES.with_borrow(|images| images.get(&name));
    if let Some(image) = stored_image {
        if image.uploaded_by != user_id {
            return Err("Access denied: You do not own this image.".to_string());
        }

        let host = "icp-api.shootify.io";
        let url = "https://icp-api.shootify.io/api/v1/utils/icp-proxy/";
        let boundary = "boundary123";
        let idempotency_key = "UUID-123456789";

        ic_cdk::println!("Start crawling for image: '{}'", name);

        let request_headers = vec![
            HttpHeader {
                name: "Host".to_string(),
                value: format!("{host}:443"),
            },
            HttpHeader {
                name: "User-Agent".to_string(),
                value: "demo_HTTP_POST_canister".to_string(),
            },
            HttpHeader {
                name: "Idempotency-Key".to_string(),
                value: idempotency_key.to_string(),
            },
            HttpHeader {
                name: "Content-Type".to_string(),
                value: format!("multipart/form-data; boundary={}", boundary),
            },
        ];

        let body = format!(
            "--{}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"{}\"\r\nContent-Type: image/jpeg\r\n\r\n",
            boundary, name
        );

        let mut body_bytes = body.into_bytes();
        body_bytes.extend_from_slice(&image.content);
        body_bytes.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

        // Calculate the size of the body and expected response
        let request_body_size = body_bytes.len() as u64;
        let expected_response_size = 20_000_000; // Adjust based on expected response size

        const BASE_CYCLES: u64 = 20_000_000_000; // Fixed cost for the request
        const BODY_COST_PER_BYTE: u64 = 200; // Cost per byte of the request body
        const RESPONSE_COST_PER_BYTE: u64 = 200; // Cost per byte of the expected response

        // Calculate the cycles required
        let cycles = BASE_CYCLES
            + request_body_size * BODY_COST_PER_BYTE
            + expected_response_size * RESPONSE_COST_PER_BYTE;

        // Clone body_bytes to avoid moving
        let body_clone = body_bytes.clone();

        let request = CanisterHttpRequestArgument {
            url: url.to_string(),
            max_response_bytes: None,
            method: HttpMethod::POST,
            headers: request_headers,
            body: Some(body_clone),
            transform: Some(TransformContext::from_name(
                "transform".to_string(),
                serde_json::to_vec(&Context {
                    bucket_start_time_index: 0,
                    closing_price_index: 4,
                }).unwrap(),
            )),
        };

        ic_cdk::println!("Estimated cycles: '{}'", cycles);

        match http_request(request, cycles.into()).await {
            Ok((response,)) => {
                let str_body = String::from_utf8(response.body)
                    .map_err(|_| "Failed to parse UTF-8 response.".to_string())?;

                let mut parsed_result: CrawlResult = serde_json::from_str(&str_body)
                    .map_err(|_| "Failed to parse crawl result.".to_string())?;

                // Assign the prediction_id explicitly
                parsed_result.prediction_id = prediction_id.clone();
                parsed_result.set_last_update_to_now();

                // Store the result
                store_crawl_result(user_id.clone(), name.clone(), parsed_result.clone())
                    .map_err(|e| format!("Failed to store crawl result: {}", e))?;

                // Serialize the response as JSON
                let response = serde_json::to_string(&parsed_result)
                    .map_err(|_| "Failed to serialize response.".to_string())?;

                ic_cdk::println!("Ok response: '{}'", response);
                Ok(response)
            }
            Err((r, m)) => {
                let message = format!("HTTP request failed. RejectionCode: {r:?}, Error: {m}");
                Err(message)
            }
        }
    } else {
        Err(format!("Image '{}' not found.", name))
    }
}


#[ic_cdk::update]
async fn detect_image_with_content(
    user_id: String,
    prediction_id: String,
    name: String,
    content: Vec<u8>,
) -> Result<String, String> {
    if user_id.is_empty() {
        return Err("User ID cannot be empty.".to_string());
    }
    if name.is_empty() {
        return Err("Image name cannot be empty.".to_string());
    }
    if content.is_empty() {
        return Err("Image content cannot be empty.".to_string());
    }

    let url = "https://icp-api.shootify.io/api/v1/utils/icp-proxy/";
    let boundary = "boundary123";

    ic_cdk::println!("Start crawling for image: '{}'", name);

    let request_headers = vec![
        HttpHeader {
            name: "User-Agent".to_string(),
            value: "demo_HTTP_POST_canister".to_string(),
        },
        HttpHeader {
            name: "Idempotency-Key".to_string(),
            value: prediction_id.clone(),
        },
        HttpHeader {
            name: "Content-Type".to_string(),
            value: format!("multipart/form-data; boundary={}", boundary),
        },
    ];

    let body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"{}\"\r\nContent-Type: image/jpeg\r\n\r\n",
        boundary, name
    );

    let mut body_bytes = body.into_bytes();
    body_bytes.extend_from_slice(&content);
    body_bytes.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    // Calculate the size of the body and expected response
    let request_body_size = body_bytes.len() as u64;
    let expected_response_size = 20_000_000; // Adjust based on expected response size

    const BASE_CYCLES: u64 = 20_000_000_000; // Fixed cost for the request
    const BODY_COST_PER_BYTE: u64 = 400; // Cost per byte of the request body
    const RESPONSE_COST_PER_BYTE: u64 = 400; // Cost per byte of the expected response

    // Calculate the cycles required
    let cycles = BASE_CYCLES
        + request_body_size * BODY_COST_PER_BYTE
        + expected_response_size * RESPONSE_COST_PER_BYTE;

    let context = Context {
        bucket_start_time_index: 0,
        closing_price_index: 4,
    };

    let request = CanisterHttpRequestArgument {
        url: url.to_string(),
        max_response_bytes: None,
        method: HttpMethod::POST,
        headers: request_headers,
        body: Some(body_bytes),
        transform: Some(TransformContext::from_name(
            "transform".to_string(),
             serde_json::to_vec(&context).unwrap(),
                                )),

        };

    ic_cdk::println!("Estimated cycles: '{}'", cycles);

    match http_request(request, cycles.into()).await {
        Ok((response,)) => {
           ic_cdk::api::print(format!("Raw response: {:?}", response.clone()));
            let str_body = String::from_utf8(response.body)
                .map_err(|_| "Failed to parse UTF-8 response.".to_string())?;

            let mut parsed_result: CrawlResult = serde_json::from_str(&str_body)
                .map_err(|_| "Failed to parse crawl result.".to_string())?;

            // Assign the prediction_id explicitly
            parsed_result.prediction_id = prediction_id.clone();
            parsed_result.set_last_update_to_now();

            // Store the result
            store_crawl_result(user_id.clone(), name.clone(), parsed_result.clone())
                .map_err(|e| format!("Failed to store crawl result: {}", e))?;

            // Serialize the response as JSON
            let response = serde_json::to_string(&parsed_result)
                .map_err(|_| "Failed to serialize response.".to_string())?;

            ic_cdk::println!("Ok response: '{}'", response);
            Ok(response)
        }
        Err((r, m)) => {
            let message = format!("HTTP request failed. RejectionCode: {r:?}, Error: {m}");
            Err(message)
        }
    }
}


#[ic_cdk::query]
fn transform(raw: TransformArgs) -> HttpResponse {
    ic_cdk::println!("Start transformation function");
    ic_cdk::println!("Raw transform arguments: {:#?}", raw);

    let headers = vec![
        HttpHeader {
            name: "Content-Security-Policy".to_string(),
            value: "default-src 'self'".to_string(),
        },
        HttpHeader {
            name: "Referrer-Policy".to_string(),
            value: "strict-origin".to_string(),
        },
        HttpHeader {
            name: "Permissions-Policy".to_string(),
            value: "geolocation=(self)".to_string(),
        },
        HttpHeader {
            name: "Strict-Transport-Security".to_string(),
            value: "max-age=63072000".to_string(),
        },
        HttpHeader {
            name: "X-Frame-Options".to_string(),
            value: "DENY".to_string(),
        },
        HttpHeader {
            name: "X-Content-Type-Options".to_string(),
            value: "nosniff".to_string(),
        },
    ];

    let mut res = HttpResponse {
        status: raw.response.status.clone(),
        body: vec![],
        headers,
        ..Default::default()
    };

    if res.status == Nat::from(200u32) {
        if let Ok(original_value) = serde_json::from_slice::<serde_json::Value>(&raw.response.body) {
            // Only copy these fields over into a new JSON object
            let fields_to_keep = [
                "created_at",
                "full_matching_images",
                "id",
                "pages_with_matching_images",
                "updated_at",
                "visually_similar_images",
                "web_entities",
            ];

            // If the top-level value is a JSON object, copy only the fields we want
            if let Some(obj) = original_value.as_object() {
                let mut new_map = serde_json::Map::new();

                // For each field we want, if it exists in obj, copy it
                for key in &fields_to_keep {
                    if let Some(value) = obj.get(*key) {
                        new_map.insert((*key).to_string(), value.clone());
                    }
                }

                // Convert that new map back to a serde_json::Value
                let filtered_value = serde_json::Value::Object(new_map);

                // Serialize that filtered JSON into bytes for the response
                if let Ok(filtered_body_bytes) = serde_json::to_vec(&filtered_value) {
                    res.body = filtered_body_bytes;
                }
            } else {
                // The JSON wasn't an object (maybe an array or string?), so just leave it unchanged or handle it as you wish
            }
        }
    } else {
        ic_cdk::println!("Error during the Transform Function (status != 200)");
    }

    ic_cdk::println!("res: {:?}", res);
    res
}
