module Exhibit exposing (..)

import Http
import Json.Decode exposing (Decoder, field, map, string)



-- REST response from the server when we authenticate to the presentation
-- that tells us where our websocket is


type alias JoinPresentationResponse =
    { url : String }



-- REST Decoders
joinPresentationResponseDecoder : Decoder JoinPresentationResponse
joinPresentationResponseDecoder =
    map JoinPresentationResponse
        (field "url" string)



{- This code is duplicated across Join and Present. It should be able to be abstracted
   I just can't figure it out yet.
-}


connectToPresentation : String -> (Result Http.Error a -> a) -> Decoder a -> Cmd a
connectToPresentation registration_key msg decoder =
    Http.post
        { url = "/join"
        , body = Http.stringBody "application/text" registration_key
        , expect = Http.expectJson msg decoder
        }


nestWebsocketMessageDecoder : String -> Decoder a -> Decoder a
nestWebsocketMessageDecoder nest decoder =
    field nest decoder
