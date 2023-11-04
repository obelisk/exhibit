module ServerMessageTypes exposing (..)
import Json.Decode exposing (Decoder)
import Json.Decode exposing (field)
import Json.Decode exposing (map2)
import Json.Decode exposing (string)
import Json.Decode exposing (map)



-- REST response from the server when we authenticate to the presentation
-- that tells us where our websocket is
type alias JoinPresentationResponse =
    { url : String }

-- Websocket Types and Subtypes
type ReceivedMessage
    = NewSlideMessage SlideSettings
    | InitialPresentationDataMessage InitialPresentationData
    | DisconnectMessage String
--| RatelimiterResponse
--| Disconnect
--| NewPoll


-- Message from the server when a new slide is shown. This needs to mirror
-- the rust type variation OutgoingUserMessage::NewSlide
type alias SlideSettings =
    { message : String
    , emojis : List String
    }


type alias Poll =
    {}



type alias InitialPresentationData =
    { title : String, settings : Maybe SlideSettings }


-- REST Decoders
joinPresentationResponseDecoder : Decoder JoinPresentationResponse
joinPresentationResponseDecoder =
    map JoinPresentationResponse
        (field "url" string)


-- WebSocket Decoders
receivedWebsocketMessageDecorder : Decoder ReceivedMessage
receivedWebsocketMessageDecorder =
    Json.Decode.oneOf
        [ Json.Decode.map NewSlideMessage newSlideMessageDecoder
        , Json.Decode.map InitialPresentationDataMessage initialPresentationDataMessageDecoder
        , Json.Decode.map DisconnectMessage disconnectMessageDecoder
        ]


nestWebsocketMessageDecoder : String -> Decoder a -> Decoder a
nestWebsocketMessageDecoder nest decoder =
    field nest decoder


newSlideMessageDecoder : Decoder SlideSettings
newSlideMessageDecoder =
    nestWebsocketMessageDecoder "NewSlide" slideSettingDecoder


slideSettingDecoder : Decoder SlideSettings
slideSettingDecoder =
    map2 SlideSettings
        (field "message" string)
        (field "emojis" (Json.Decode.list string))


initialPresentationDataMessageDecoder : Decoder InitialPresentationData
initialPresentationDataMessageDecoder =
    nestWebsocketMessageDecoder "InitialPresentationData"
        (map2 InitialPresentationData
            (field "title" string)
            (field "settings" (Json.Decode.maybe slideSettingDecoder))
        )

disconnectMessageDecoder : Decoder String
disconnectMessageDecoder =
    field "Disconnect" string