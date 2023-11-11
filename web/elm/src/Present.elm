port module Present exposing (..)
import Browser

import Exhibit exposing (..)
import Html exposing (Html)
import Html exposing (div)
import Html.Attributes exposing (class)
import Html exposing (label)
import Html.Attributes exposing (for)
import Html exposing (text)
import Html exposing (input)
import Html.Attributes exposing (type_)
import Html.Attributes exposing (id)
import Html.Attributes exposing (value)
import Html.Events exposing (onInput)
import Html exposing (button)
import Html.Events exposing (onClick)
import Html exposing (img)
import Html.Attributes exposing (multiple)
import Http
import Browser.Events
import Json.Decode as Decode
import Html.Events exposing (on)
import File exposing (..)
import Dict exposing (Dict)
import Json.Decode exposing (field)
import Json.Decode exposing (string)
import Task exposing (..)

-- Ports
port socketConnect : String -> Cmd msg
port sendMessage : String -> Cmd msg
port messageReceived : (String -> msg) -> Sub msg
port socketDisconnected : (String -> msg) -> Sub msg

type State = Disconnected
    | Joining
    | Authenticated JoinPresentationResponse
    --| Presenting Presentation

type Key
  = Character Char
  | Control String

type Msg
    -- Handle messages around connections and general
    -- house keeping
    = ChangeRegistrationKey String
    | GetSlideData (Cmd Msg)
    | SlideDataRead (String, Dict String String)
    | SlideDataError String
    | AuthenticateToPresentation
    | GotWebsocketAddress (Result Http.Error JoinPresentationResponse)
    | StartPresentation String
    | ReceivedWebsocketMessage String
    | SocketDisconnected String
    -- Handle events for running the presentation
    | AddSlideData (Maybe Slides)
    | NextSlide
    | PreviousSlide
    | OtherKey String

type alias Poll = {}

type alias SlideData = 
    { message: String
    , emojis: List String
    --, poll: Maybe Poll
    }

slideDataDecoder : Decode.Decoder SlideData
slideDataDecoder = 
    Decode.map2 SlideData
        (field "message" string)
        (field "emoji" (Json.Decode.list string))
        --(field "vote_type" voteTypeDecoder)
    
{-
voteTypeDecoder : Decoder VoteType
voteTypeDecoder = 
    (Json.Decode.oneOf
        [ Json.Decode.map SingleBinary (field "SingleBinary" (field "choice" string))
        , Json.Decode.map MultipleBinary (field "MultipleBinary" (field "choices" (Json.Decode.dict Json.Decode.bool)))
        ])
-}


type alias Slide =
    { slide: File
    , data: SlideData
    }

type alias Slides =
    { past_slides: List Slide
    , future_slides: List Slide
    }

type alias Model =
    { registration_key : String
    , error : Maybe String
    , slides : Slides
    , state : State
    }


keyDecoder : Decode.Decoder Msg
keyDecoder =
    Decode.map toKey (Decode.field "key" Decode.string)

toKey : String -> Msg
toKey string =
    case string of
        "ArrowLeft" ->
            PreviousSlide
        "ArrowRight" ->
            NextSlide
        x -> OtherKey x

init : () -> ( Model, Cmd Msg )
init _ =
    (
        { registration_key = ""
        , error = Nothing
        , slides = {past_slides = [], future_slides =  []}
        , state = Disconnected
        },
    Cmd.none )

subscriptions : Model -> Sub Msg
subscriptions _ =
    Sub.batch
        [ messageReceived ReceivedWebsocketMessage
        , socketDisconnected SocketDisconnected
        , Browser.Events.onKeyDown keyDecoder
        ]

main =
    Browser.element { init = init, update = update, subscriptions = subscriptions, view = view }

update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        GetSlideData _ -> (model, Cmd.none)
        SlideDataRead _ -> (model, Cmd.none)
        SlideDataError _ -> (model, Cmd.none)
        AddSlideData slides ->
            let _ = Debug.log "Slides" slides in
                case slides of
                    Just s ->
                        ( { model | slides = s }, Cmd.none )
                    Nothing ->
                        ( model, Cmd.none )
        ChangeRegistrationKey newRegistrationKey ->
            ( { model | registration_key = newRegistrationKey }, Cmd.none )

        AuthenticateToPresentation ->
            if model.state == Disconnected then
                ( model
                ,     Http.post
                        { url = "/join"
                        , body = Http.stringBody "application/text" model.registration_key
                        , expect = Http.expectJson GotWebsocketAddress joinPresentationResponseDecoder
                        }
                )
            else
                ( model, Cmd.none )
        
        GotWebsocketAddress response ->
            case response of
                -- We successfully authenticated to the presentation,
                -- open the websocket via the port
                Ok startPresentationResponse ->
                    update (StartPresentation startPresentationResponse.url) { model | state = Authenticated startPresentationResponse }

                -- Should try and do something here to notify the presenter something went wrong
                Err _ ->
                    ( model, Cmd.none )
        
        -- Handle the response from the REST API with our websocket address
        -- We need to send a message to the port even before the websocket is
        -- open to force Elm to create it.
        StartPresentation url ->
            ( model, Cmd.batch [ socketConnect url, sendMessage "Hello" ] )

        ReceivedWebsocketMessage message ->
            ( model, Cmd.none )

        -- Reconnect to the presentation if disconnected
        SocketDisconnected _ ->
            update AuthenticateToPresentation { model | state = Disconnected }
        NextSlide ->
            ( model, Cmd.none )
        PreviousSlide ->
            ( model, Cmd.none )
        OtherKey _ ->
            ( model, Cmd.none )


filesDecoderMsg: Decode.Decoder Msg
filesDecoderMsg =
    (Decode.at ["target", "files"] (Decode.map intoSlides (Decode.list File.decoder)))

intoSlides: (List File) -> Msg
intoSlides files =
    let 
        data_files = (List.filter (\file -> (String.endsWith (name file) ".json")) files)
        image_files = (List.filter (\file -> (String.endsWith (name file) ".png")) files) in
        buildGetSlidesTask data_files (organizeSlideImages image_files)

-- parseSlideData: List File -> Maybe SlideData
-- parseSlideData data_files =
--     case ((List.length data_files), List.head data_files) of
--         (1, Just data_file) 
--             -> let data = (toString data_file) in
--                 case Json.Decode.decodeString Json.Decode.string data of
--                 _ -> Nothing
--         _ -> Nothing

organizeSlideImages: List File -> Dict String File
organizeSlideImages slides =
    Dict.fromList (List.map (\file -> (name file, file)) slides)

buildGetSlidesTask: List File -> Dict String File -> Msg
buildGetSlidesTask data_files image_files =
    -- In the event there is exactly one json data file
    case (List.head data_files) of
        Just data_file -> GetSlideData (
            perform SlideDataRead (buildFileReadingTask data_file image_files)
            )
        _ -> SlideDataError "There was more than one data file (JSON) selected."

-- (Dict.toList (Dict.map (\file -> (toString file)) image_files))
buildFileReadingTask: File -> Dict String File -> Task Never (String, Dict String String)
buildFileReadingTask data image_files =
    Task.map2 (\slide_data slide_images -> (slide_data, Dict.fromList slide_images)) (toString data) (Task.sequence (List.map (\(slide_name, slide_file) -> (toString slide_file) |> andThen (\image_contents -> (succeed (slide_name, image_contents)))) (Dict.toList image_files)))

view : Model -> Html Msg
view model =
    div [ class "container" ] [
        label [ for "registration_key" ] [ text "Registration Key:" ]
    ,   input [ type_ "text", id "registration_key", value model.registration_key, onInput ChangeRegistrationKey ] []
    ,   button [ onClick AuthenticateToPresentation ] [ text "Start Presentation" ]
    ,   input [ type_ "file" , multiple True, on "change" filesDecoderMsg ] []
    ,   div [ id "slides-container"] [
        img [ id "slide-img" ] []
    ]
    ,   div [id "reactions-float-bottom" ] [ 
            div [ id "reactions-container"] [] 
    ]
    ]