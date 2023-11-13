module ServerMessagePresenterTypes exposing (..)
import Json.Decode exposing (Decoder)
import Exhibit exposing (nestWebsocketMessageDecoder)
import Dict exposing (Dict)


type ReceivedMessage
    = Emoji EmojiMessage
    | PollResults(Dict String Int)
    | Error String

type alias EmojiMessage =
    { emoji : String
    , size : Int
    }


emojiMessageDecoder : Decoder EmojiMessage
emojiMessageDecoder =
    Json.Decode.map2 EmojiMessage
        (Json.Decode.field "emoji" Json.Decode.string)
        (Json.Decode.field "size" Json.Decode.int)


receivedWebsocketMessageDecorder : Decoder ReceivedMessage
receivedWebsocketMessageDecorder =
    Json.Decode.oneOf
        [ Json.Decode.map Emoji (nestWebsocketMessageDecoder "Emoji" emojiMessageDecoder)
        , Json.Decode.map PollResults (nestWebsocketMessageDecoder "PollResults" (Json.Decode.dict Json.Decode.int))
        , Json.Decode.map Error (nestWebsocketMessageDecoder "Error" Json.Decode.string)
        ]