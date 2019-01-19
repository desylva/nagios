package main

// Checks an URL redirects correctly to another

import (
	"fmt"
	"net/http"
	"os"
)

func main() {
	countArgs := len(os.Args)
	if countArgs < 3 {
		fmt.Println("Error: Expects 'target url' and 'target url' as first arguments")
		os.Exit(1)
	}

	targetURL := os.Args[1]
	expectedURL := os.Args[2]
	var host string
	if countArgs > 3 {
		host = os.Args[3]
	}

	client := &http.Client{}
	req, _ := http.NewRequest("GET", targetURL, nil)
	if len(host) > 0 {
		req.Header.Add("Host", host)
	}
	resp, err := client.Do(req)
	if err != nil {
		fmt.Println("Error: ", err)
		os.Exit(1)
	}

	retrievedURL := resp.Request.URL.String()

	if retrievedURL != expectedURL {
		fmt.Printf("WARNING: Target url: %v . Expected url: %v . Returns url %v !", targetURL, expectedURL, retrievedURL)
		os.Exit(1)
	}

	fmt.Printf("OK: Returns url %v", retrievedURL)
	os.Exit(0)
}
