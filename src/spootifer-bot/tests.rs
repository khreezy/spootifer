#[cfg(test)]
mod tests {
    use crate::spotify::*;
    use std::env;
    
    #[tokio::test]
    async fn test_album_cover_image_retrieval_from_track() {
        // Skip test if no Spotify credentials are set
        if env::var("RSPOTIFY_CLIENT_ID").is_err() || env::var("RSPOTIFY_CLIENT_SECRET").is_err() {
            println!("Skipping integration test - Spotify credentials not set");
            return;
        }
        
        // Test with a known Spotify track link (Abbey Road - Come Together)
        let test_link = "https://open.spotify.com/track/2EqlS6tkEnglzr7tkKAAYD";
        
        // Extract track ID from the link
        let track_id = extract_track_id(test_link).expect("Failed to extract track ID");
        
        // Initialize Spotify client (requires credentials)
        let spotify = init_spotify().expect("Failed to initialize Spotify client");
        
        // Get album cover image from track
        let cover_image = get_album_cover_image_from_track(&spotify, &track_id).await
            .expect("Failed to retrieve album cover image from track");
        
        // Verify we got an image
        assert!(cover_image.is_some(), "Expected to find album cover image");
        
        let image = cover_image.unwrap();
        assert!(!image.url.is_empty(), "Image URL should not be empty");
        assert!(image.url.starts_with("https://"), "Image URL should be HTTPS");
        
        println!("✓ Successfully retrieved album cover image from track: {}", image.url);
        println!("  Dimensions: {}x{}", image.width.unwrap_or(0), image.height.unwrap_or(0));
    }
    
    #[test]
    fn test_extract_track_id() {
        let test_cases = vec![
            ("https://open.spotify.com/track/4yP0hdKOZPNshxUOjY0cZj", Some("4yP0hdKOZPNshxUOjY0cZj")),
            ("https://open.spotify.com/track/2EqlS6tkEnglzr7tkKAAYD", Some("2EqlS6tkEnglzr7tkKAAYD")),
            ("https://open.spotify.com/album/4yP0hdKOZPNshxUOjY0cZj", None),
            ("https://example.com/track/123", None),
            ("invalid_link", None),
        ];
        
        for (input, expected) in test_cases {
            let result = extract_track_id(input);
            assert_eq!(result.as_deref(), expected, "Failed for input: {}", input);
        }
    }
    
    #[test]
    fn test_is_album() {
        assert!(is_album("https://open.spotify.com/album/4yP0hdKOZPNshxUOjY0cZj"));
        assert!(!is_album("https://open.spotify.com/track/4yP0hdKOZPNshxUOjY0cZj"));
        assert!(!is_album("https://example.com/album/123"));
    }

    #[tokio::test]
    async fn test_album_cover_image_retrieval() {
        // Skip test if no Spotify credentials are set
        if env::var("RSPOTIFY_CLIENT_ID").is_err() || env::var("RSPOTIFY_CLIENT_SECRET").is_err() {
            println!("Skipping integration test - Spotify credentials not set");
            return;
        }
        
        // Test with a known Spotify album link
        let test_link = "https://open.spotify.com/album/4yP0hdKOZPNshxUOjY0cZj";
        
        // Extract album ID from the link
        let album_id = extract_album_id(test_link).expect("Failed to extract album ID");
        
        // Initialize Spotify client (requires credentials)
        let spotify = init_spotify().expect("Failed to initialize Spotify client");
        
        // Get album cover image
        let cover_image = get_album_cover_image(&spotify, &album_id).await
            .expect("Failed to retrieve album cover image");
        
        // Verify we got an image
        assert!(cover_image.is_some(), "Expected to find album cover image");
        
        let image = cover_image.unwrap();
        assert!(!image.url.is_empty(), "Image URL should not be empty");
        assert!(image.url.starts_with("https://"), "Image URL should be HTTPS");
        
        println!("✓ Successfully retrieved album cover image: {}", image.url);
        println!("  Dimensions: {}x{}", image.width.unwrap_or(0), image.height.unwrap_or(0));
    }
    
    #[test]
    fn test_extract_album_id() {
        let test_cases = vec![
            ("https://open.spotify.com/album/4yP0hdKOZPNshxUOjY0cZj", Some("4yP0hdKOZPNshxUOjY0cZj")),
            ("https://open.spotify.com/album/1DFixLWuPkv3KT3TnV35m3", Some("1DFixLWuPkv3KT3TnV35m3")),
            ("https://open.spotify.com/track/4yP0hdKOZPNshxUOjY0cZj", None),
            ("https://example.com/album/123", None),
            ("invalid_link", None),
        ];
        
        for (input, expected) in test_cases {
            let result = extract_album_id(input);
            assert_eq!(result.as_deref(), expected, "Failed for input: {}", input);
        }
    }
}