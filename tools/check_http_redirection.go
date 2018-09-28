package main

import (
	"fmt"
	"net/http"
	"os"
)

func main() {
	countArgs := len(os.Args)
	if countArgs < 3 {
		fmt.Println("Error: Expects 'source url' and 'target url' as first arguments")
		os.Exit(1)
	}

	sourceUrl := os.Args[1]
	expectedUrl := os.Args[2]

	resp, err := http.Get(sourceUrl)
	if err != nil {
		fmt.Println("Error: ", err)
		os.Exit(1)
	}

	retrievedUrl := resp.Request.URL.String()

	if retrievedUrl != targetUrl {
		fmt.Printf("WARNING: Expects url %v. Returns url %v", targetUrl, retrievedUrl)
		os.Exit(1)
	}

	fmt.Printf("OK: Returns url %v", retrievedUrl)
	os.Exit(0)
}
