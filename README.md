# Scoutfile sender

<!-- badges: start -->
[![Lifecycle:Experimental](https://img.shields.io/badge/Lifecycle-Experimental-339999)](https://www.tidyverse.org/lifecycle/#experimental)
<!-- badges: end -->

A cross-platform app for monitoring a volleyball scout file and sending it to a remote server when it changes. It is intended as an open mechanism for sharing live-scouted data files (so that they can be used by coaches or others, in online apps or similar, as the match progresses).

## Security note

This app uses https://getpantry.cloud/ as the data exchange platform, because it is free to use and has a simple API for access. Note, however, that anyone who knows your pantry ID can see any file that you upload. Also be aware that once a file has been uploaded, its live data link has your pantry ID embedded in it. We therefore do not recommend using this app for uploading sensitive files. A more appropriate mechanism for sensitive data might be added at a later date, if there is a demand for it.

Science Untangled users can share live-scouted stats without exposing their pantry ID --- see "Live stats" below.

## How to use

1. Sign up to Pantry and get your pantry ID. Go to https://getpantry.cloud/ and look for the "Create a Pantry" button. It will give you a pantry ID - save this somewhere.

1. Download and install the Scoutfile sender app from the [GitHub releases page](https://github.com/scienceuntangled/file_sender/releases). Installers are available for Windows, Mac, and Linux. If you are not a Science Untangled user, you can choose the app version without the SU live app link (it will show the data link only).

1. Start the Scoutfile sender app, and then:

    - enter your pantry ID into the associated text box

    - click the `Select scout file` button and choose your scout file

    - the `Use base64 encoding` box is ticked by default, and is probably safest to leave that way. You might not need this if your file does not use any non-ASCII text (i.e. no accented, Cyrillic, kanji, or similar non-ASCII characters). Base64 encoding can cope with such text, but creates a larger file that will be slightly slower to process. If you see an error saying "stream did not contain valid UTF-8" then base64 encoding must be used.

1. The icon next to the filename will show a progress indicator each time the file is uploading, followed by a green tick if the upload was successful (or a red cross if not).


## Accessing the live data

Once you have entered your pantry ID and selected a scout file, you should see the "Data link" buttons below. The data link is the Pantry download link, which will be something like `https://getpantry.cloud/apiv1/pantry/PANTRY_ID/basket/FILE_NAME` (note that the `FILE_NAME` will be URL-encoded, which means that e.g. spaces will be replaced by "%20").

Use the buttons to copy the associated link to the clipboard or open it in a browser.

## Live stats

Science Untangled users can also open the "Live app" link. Once the live app has opened, ensure that you are logged into your Science Untangled account and then look for the "Share this session with anyone" button. This allows you to share stats from your live-scouted file with other (non-SU) users --- your coaching staff, perhaps. The app will show a QR code to allow easy opening in another (mobile) device.

### Deleting old files

Pantry allows a maximum of 100 different files to be stored at any one time. The same file uploaded multiple times (with changes) in the same session only counts as one file.

Files will automatically be deleted from your Pantry storage after 30 days, but if you need to clean up your storage you can do so via the [Pantry dashboard](https://getpantry.cloud/) (click the "Dashboard" button).

### Downloading in scripts

The file is stored in json format, optionally base64-encoded. To retrieve it in a script you need to make a GET request, then extract the data from the json packet and optionally base64-decode it. A helper function in R might look like:

```
library(base64enc)
library(curl)
library(jsonlite)
library(httr)

fetch_pantry_url <- function(url, max_size = 5e6, accept = c("text/plain", "application/json")) {
    h <- new_handle(maxfilesize = max_size) ## control the max file size we will accept
    handle_setheaders(h, Accept = accept) ## and the allowed response types
    ## for pantry, download to memory, optionally b64-decode, and write to file
    url <- URLencode(url)
    res <- curl_fetch_memory(url = url, handle = h)
    if (res$status_code == 200) {
        res <- fromJSON(rawToChar(res$content))
        if (!setequal(names(res), c("filename", "data", "last_modified"))) stop("pantry data in unexpected format")
        path <- tempfile() ## save to file in temporary directory
        dir.create(path)
        path <- file.path(path, basename(res$filename))
        ## is the data base64-encoded?
        if (grepl("^([A-Za-z0-9+/]{4})*([A-Za-z0-9+/]{3}=|[A-Za-z0-9+/]{2}==)?$", res$data)) {
            ## yes
            cat(rawToChar(base64decode(res$data)), file = path, sep = "\n")
        } else {
            cat(res$data, file = path, sep = "\n")
        }
        path
    } else {
        ermsg <- tryCatch(paste0(": ", http_status(res$status_code)), error = function(e) "")
        stop("download failed", ermsg)
    }
}

x <- fetch_pantry_url("https://getpantry.cloud/apiv1/pantry/PANTRY_ID/basket/FILE_NAME")
## will download to a temporary file and return the filename

```

