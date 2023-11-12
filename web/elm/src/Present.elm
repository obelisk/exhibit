port module Present exposing (..)

import Browser
import Browser.Events
import Dict exposing (Dict)
import Exhibit exposing (..)
import File exposing (..)
import Html exposing (Html, button, div, img, input, label, text)
import Html.Attributes exposing (class, for, id, multiple, type_, value)
import Html.Events exposing (on, onClick, onInput)
import Http
import Json.Decode as Decode exposing (field, string)
import Task exposing (..)
import Html.Attributes exposing (src)



-- Ports


port socketConnect : String -> Cmd msg


port sendMessage : String -> Cmd msg


port messageReceived : (String -> msg) -> Sub msg


port socketDisconnected : (String -> msg) -> Sub msg


type State
    = Disconnected
    | Joining
    | Authenticated JoinPresentationResponse



--| Presenting Presentation


type Key
    = Character Char
    | Control String


type
    Msg
    -- Handle messages around connections and general
    -- house keeping
    = ChangeRegistrationKey String
    | GetSlideData (Cmd Msg)
    | SlideDataRead ( String, Dict String String )
    | SlideDataError String
    | AuthenticateToPresentation
    | GotWebsocketAddress (Result Http.Error JoinPresentationResponse)
    | StartPresentation String
    | ReceivedWebsocketMessage String
    | SocketDisconnected String
      -- Handle events for running the presentation
    | NextSlide
    | PreviousSlide
    | OtherKey String


type alias Poll =
    {}


type alias SlideData =
    { slide : String
    , message : String
    , emojis : List String

    --, poll: Maybe Poll
    }


slideDataDecoder : Decode.Decoder SlideData
slideDataDecoder =
    Decode.map3 SlideData
        (field "slide" string)
        (field "message" string)
        (field "emojis" (Decode.list string))



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
    { data : SlideData
    , image : String 
    }


type alias Slides =
    { past_slides : List Slide
    , future_slides : List Slide
    }


type alias Model =
    { registration_key : String
    , status : Maybe String
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

        x ->
            OtherKey x


init : () -> ( Model, Cmd Msg )
init _ =
    ( { registration_key = ""
      , status = Nothing
      , slides = { past_slides = [], future_slides = [] }
      , state = Disconnected
      }
    , Cmd.none
    )


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
        GetSlideData read ->
            ( model, read )

        SlideDataRead ( slide_data, slide_contents ) ->
            case Decode.decodeString (Decode.list slideDataDecoder) slide_data of
                Ok sd -> case zipSlideDataAndImages sd slide_contents of
                    Just slides ->
                        ( { model
                            | status = (Just ("Ready with " ++ (String.fromInt (List.length slides)) ++ " slides loaded"))
                            , slides = { past_slides = [], future_slides = slides }
                        }, Cmd.none )
                    Nothing -> ( {model| status = Just "Could not sync up data file with images. This means slides are defined for which the images were not provided"}, Cmd.none )

                Err e -> ({model | status = Just (Decode.errorToString e)}, Cmd.none)

        SlideDataError _ -> ( model, Cmd.none )

        ChangeRegistrationKey newRegistrationKey ->
            ( { model | registration_key = newRegistrationKey }, Cmd.none )

        AuthenticateToPresentation ->
            if model.state == Disconnected then
                ( model
                , Http.post
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

        NextSlide -> case (List.head model.slides.future_slides, List.length model.slides.future_slides) of
            -- Don't allow to go past the end of the slides
            (Just _, 1) -> ( model, Cmd.none )
            -- There are still more future slides
            (Just slide, _) ->
                ( { model
                    | slides = { past_slides = slide :: model.slides.past_slides, future_slides = List.drop 1 model.slides.future_slides }
                  }
                , Cmd.none
                )
            -- There are no more future slides and catch all to keep the UI the same
            _ ->
                ( model, Cmd.none )

        PreviousSlide ->
            case List.head model.slides.past_slides of
                Just slide ->
                    ( { model
                        | slides = { past_slides = List.drop 1 model.slides.past_slides, future_slides = slide :: model.slides.future_slides }
                    }
                    , Cmd.none
                    )
                Nothing ->
                    ( model, Cmd.none )

        OtherKey _ ->
            ( model, Cmd.none )


filesDecoderMsg : Decode.Decoder Msg
filesDecoderMsg =
    Decode.at [ "target", "files" ] (Decode.map sortFiles (Decode.list File.decoder))


sortFiles : List File -> Msg
sortFiles files =
    let
        data_files =
            List.filter (\file -> String.endsWith ".json" (name file)) files

        image_files =
            List.filter (\file -> String.endsWith ".png" (name file)) files
    in
    buildGetSlidesTask data_files (organizeSlideImages image_files)

zipSlideDataAndImages: List SlideData -> Dict String String -> Maybe (List Slide)
zipSlideDataAndImages slide_data slide_images =
    List.foldl
        (\potential slides ->
            case (slides, Dict.get potential.slide slide_images) of
                (Just s, Just image_data) -> Just (s ++ [{data = potential, image = image_data}])
                _ -> Nothing
        )
        (Just [])
        slide_data

organizeSlideImages : List File -> Dict String File
organizeSlideImages slides =
    Dict.fromList (List.map (\file -> ( name file, file )) slides)


buildGetSlidesTask : List File -> Dict String File -> Msg
buildGetSlidesTask data_files image_files =
    -- In the event there is exactly one json data file
    case data_files of
        [] ->
            SlideDataError "There was no data file (JSON) selected."

        [ data_file ] ->
            GetSlideData (perform SlideDataRead (buildFileReadingTask data_file image_files))

        _ ->
            SlideDataError "There was more than one data file (JSON) selected."


buildFileReadingTask : File -> Dict String File -> Task Never ( String, Dict String String )
buildFileReadingTask data image_files =
    Task.map2 
        (\slide_data slide_images -> ( slide_data, Dict.fromList slide_images ))
        (toString data)
        (Task.sequence 
            (List.map
                (\( slide_name, slide_file ) -> toUrl slide_file |> andThen (\image_contents -> succeed ( slide_name, image_contents )))
                (Dict.toList image_files)
            )
        )


view : Model -> Html Msg
view model =
    div [ class "container" ]
        [ label [ for "registration_key" ] [ text "Registration Key:" ]
        , input [ type_ "text", id "registration_key", value model.registration_key, onInput ChangeRegistrationKey ] []
        , case model.status of
            Just status ->
                div [ class "status" ] [ text status ]

            Nothing ->
                div [] []
        , button [ onClick AuthenticateToPresentation ] [ text "Start Presentation" ]
        , input [ type_ "file", multiple True, on "change" filesDecoderMsg ] []
        , div [ id "slides-container" ] [
            case List.head model.slides.future_slides of
                Just slide ->
                     img [ id "slide-img", src slide.image] []
                Nothing -> div [] []
            ]
        , div [ id "reactions-float-bottom" ]
            [ div [ id "reactions-container" ] []
            ]
        ]
