module UserMessageTypes exposing (..)

import Json.Encode



-- Messages from the user are objects under the user key
-- to denote what objects they can be deserialized into on
-- the server size. Here we can easily wrap all the types


type alias UserMessage a =
    { user : a }


encodeUserMessage : (a -> Json.Encode.Value) -> a -> Json.Encode.Value
encodeUserMessage encoder message =
    Json.Encode.object
        [ ( "User", encoder message )
        ]

encodeEmojiReaction : String -> Int -> String
encodeEmojiReaction emoji size =
    Json.Encode.encode 0
        (encodeUserMessage Json.Encode.object
            [ ( "Emoji"
              , Json.Encode.object
                    [ ( "emoji", Json.Encode.string emoji )
                    , ( "size", Json.Encode.int size )
                    ]
              )
            ]
        )
