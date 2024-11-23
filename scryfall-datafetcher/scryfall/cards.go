package scryfall

import (
	"encoding/json"
	"io"
	"log"
	"net/http"
)

type BulkData struct {
	Data []BulkEntry `json:"data"`
}

type BulkEntry struct {
	Type string `json:"type"`
	Url  string `json:"download_uri"`
}

type Card struct {
	Name   string `json:"name"`
	Images struct {
		ArtCrop string `json:"art_crop,omitempty"`
	} `json:"image_uris,omitempty"`
	Set         string `json:"set_name"`
	FlavourText string `json:"flavor_text,omitempty"`
	Artist      string `json:"artist,omitempty"`
}

func (c *Card) isInvalid() bool {
	switch c.Set {
	case "Unglued":
		return true
	case "Unhinged":
		return true
	case "Unstable":
		return true
	case "Unsanctioned":
		return true
	case "Unfinity":
		return true
	default:
		break
	}

	if c.Set == "Unknown Event" {
		return true
	}

	if c.FlavourText == "" || c.Images.ArtCrop == "" || c.Artist == "" {
		return true
	}

	return false
}

const URL = "https://api.scryfall.com/bulk-data"

func downloadData(url string) ([]byte, error) {
	raw, err := http.Get(url)
	if err != nil {
		log.Fatalf("unable to download data from %s: %v\n", url, err)
	}

	defer raw.Body.Close()
	return io.ReadAll(raw.Body)
}

func downloadBulkData() BulkData {
	data, err := downloadData(URL)
	var bulk BulkData
	err = json.Unmarshal(data, &bulk)
	if err != nil {
		log.Fatalf("unable to decode bulk data response: %v\n", err)
	}

	log.Println("downloaded bulk card data")
	return bulk
}

func downloadCardData(entry BulkEntry) []Card {
	data, err := downloadData(entry.Url)
	if err != nil {
		log.Fatalf("unable to download card data from %s: %v\n", entry.Url, err)
	}

	var cards []Card
	err = json.Unmarshal(data, &cards)
	if err != nil {
		log.Fatalf("unable to deserialize card list: %v\n", err)
	}

	log.Println("downloaded oracle card data")

	return filterCards(cards)
}

func filterCards(cards []Card) []Card {
	filtered := make([]Card, 0)
	for _, card := range cards {
		if card.isInvalid() {
			continue
		}
		filtered = append(filtered, card)
	}

	log.Println("filtered cards")
	return filtered
}

func Download() []Card {
	oracle := downloadBulkData().Data[0]
	return downloadCardData(oracle)
}
