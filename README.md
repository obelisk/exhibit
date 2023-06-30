# Exhibit
A simple piece of interactive presentation software allowing live emoji reactions.

## How It Works
1. Create a presentation
2. Export the presentation as a series of images
3. Provide the images and an emoji configuration to the presenter
4. Clients connect
5. Emoji time

## Deployment
Exhibit starts a public webserver that requires a fronting authentication proxy such as nginx to provide the X-SSO-EMAIL header that identifies users. There is a client single page app at the root that opens a persistent websocket connection to send emojis.

The server will start an additional private webserver used for showing the presentation. This server is not designed to be exposed publicly so if running on a remote server, SSH tunnels or VPNs will need to be used to access the presentation single page app.

## Exporting a Presentation
When using Google slides, exporting as a PDF then converting that PDF using Imagemagick is the easiest way I've found though it can be quite slow. Here is an example command to do that:

```
convert -density 350 Your-Amazing-Slide-Deck.pdf -quality 100 exhibit/slide.png
```