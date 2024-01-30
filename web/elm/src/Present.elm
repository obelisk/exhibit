port module Present exposing (..)

import Browser
import Browser.Events
import Dict exposing (Dict)
import Exhibit.IO exposing (..)
import File exposing (..)
import Html exposing (Html, button, div, img, input, label, text, span)
import Html.Attributes exposing (class, for, id, multiple, type_, value, classList)
import Html.Events exposing (on, onClick, onInput)
import Http
import Json.Decode as Decode exposing (field, string)
import Task exposing (..)
import Html.Attributes exposing (src)
import Json.Encode
import Exhibit.ServerMessagePresenterTypes exposing (receivedWebsocketMessageDecoder, ReceivedMessage(..), EmojiMessage)
import Json.Decode exposing (errorToString)
import Exhibit.UserMessageTypes exposing (encodeVoteType)
import Process
import Exhibit.Visualizations.Centroid
import Html.Attributes exposing (style)
import Svg.Attributes exposing (mode)



-- Ports


port socketConnect : String -> Cmd msg


port sendMessage : String -> Cmd msg


port addAnimatedEmoji : ( String, Int ) -> Cmd msg


port messageReceived : (String -> msg) -> Sub msg


port socketDisconnected : (String -> msg) -> Sub msg


type State
    = Disconnected
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
    | KillSwitch
    | ToggleAspect
    | OtherKey String
    -- Handle automatic events generated by the presentation
    | UpdatePollResults


encodePresenterMessage : (a -> Json.Encode.Value) -> a -> Json.Encode.Value
encodePresenterMessage encoder message =
    Json.Encode.object
        [ ( "Presenter", encoder message )
        ]

type alias Poll =
    { name : String
    , options : List String
    , vote_type : VoteType
    }

encodePollAsNewPollMessage : Poll -> String
encodePollAsNewPollMessage poll = 
    Json.Encode.encode 0
        (encodePresenterMessage Json.Encode.object
            [ ( "NewPoll"
              , Json.Encode.object
                    [ ( "name", Json.Encode.string poll.name )
                    , ( "options", Json.Encode.list Json.Encode.string poll.options )
                    , ( "vote_type", encodeVoteType poll.vote_type )
                    ]
                )
            ]
        )

type alias PollRender =
    { refresh_interval : Int
    , type_ : String
    , x : Int
    , y : Int
    , scale : Int
    , vbx : Int
    , vby : Int
    }

pollRenderDecoder : Decode.Decoder PollRender
pollRenderDecoder = 
    Decode.map7 PollRender
        (field "refresh_interval" Decode.int)
        (field "type_" Decode.string)
        (field "x" Decode.int)
        (field "y" Decode.int)
        (field "scale" Decode.int)
        (field "vbx" Decode.int)
        (field "vby" Decode.int)

type alias SlideData =
    { poll : Maybe Poll
    , poll_render : Maybe PollRender 
    , addRateLimiter : Maybe Decode.Value
    , removeRateLimiter : Maybe String
    , slide : String
    , message : String
    , emojis : List String
    }

encodeSlideDataAsNewSlideMessage : SlideData -> Int -> String
encodeSlideDataAsNewSlideMessage sd index =
    Json.Encode.encode 0
        (encodePresenterMessage Json.Encode.object
            [ ( "NewSlide"
              , Json.Encode.object
                    [ ( "slide", Json.Encode.int index )
                    , ( "slide_settings", Json.Encode.object
                        [ ( "message", Json.Encode.string sd.message )
                        , ( "emojis", Json.Encode.list Json.Encode.string sd.emojis )
                        ]
                      )
                    ]
                )
            ]
        )

encodeAddRateLimiterAsNewRateLimiterMessage : Decode.Value -> String
encodeAddRateLimiterAsNewRateLimiterMessage limiterRaw =
    Json.Encode.encode 0
        (encodePresenterMessage Json.Encode.object
            [ ( "AddRatelimiter", limiterRaw )
            ]
        )

encodeRemoveRateLimiterAsRemoveRateLimiterMessage : String -> String
encodeRemoveRateLimiterAsRemoveRateLimiterMessage rateLimiterName =
    Json.Encode.encode 0
        (encodePresenterMessage Json.Encode.object
            [ ( "RemoveRatelimiter", Json.Encode.object
                [ ( "name", Json.Encode.string rateLimiterName )
                ] 
            )
            ]
        )

slideDataDecoderNoPoll : Decode.Decoder SlideData
slideDataDecoderNoPoll =
    Decode.map3 (SlideData Nothing Nothing Nothing Nothing)
        (field "slide" string)
        (field "message" string)
        (field "emojis" (Decode.list string))

decodeSlideJSON : Decode.Decoder SlideData
decodeSlideJSON =
    Decode.map7 SlideData
        (Decode.maybe (field "poll" pollDecoder))
        (Decode.maybe (field "poll_render" pollRenderDecoder))
        (Decode.maybe (field "addRateLimiter" Decode.value))
        (Decode.maybe (field "removeRateLimiter" Decode.string))
        (field "slide" string)
        (field "message" string)
        (field "emojis" (Decode.list string))


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
    , emojis : List EmojiMessage
    , poll_results : Dict String Int
    , poll_render : Maybe PollRender
    , killswitch_count : Int
    , killed : Bool
    , stretchedMode : Bool
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

        "ArrowUp" ->
            KillSwitch

        "A" ->
            ToggleAspect

        x ->
            OtherKey x


init : Maybe String -> ( Model, Cmd Msg )
init registration_key =
    ( { registration_key = Maybe.withDefault "" registration_key
      , status = Nothing
      , slides = { past_slides = [], future_slides = [] }
      , state = Disconnected
      , emojis = []
      , poll_results = Dict.empty
      , poll_render = Nothing
      , killswitch_count = 0
      , killed = False
      , stretchedMode = True
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

delay : Int -> msg -> Cmd msg
delay time msg =
    -- create a task that sleeps for `time`
    Process.sleep (toFloat time * 1000)
        |> -- once the sleep is over, ignore its output (using `always`)
           -- and then we create a new task that simply returns a success, and the msg
           Task.andThen (always <| Task.succeed msg)
        |> -- finally, we ask Elm to perform the Task, which
           -- takes the result of the above task and
           -- returns it to our update function
           Task.perform identity

main =
    Browser.element { init = init, update = update, subscriptions = subscriptions, view = view }


getCurrentPoll: Model -> Maybe Poll
getCurrentPoll model =
    (List.head model.slides.future_slides)
        |> Maybe.andThen (\slide -> slide.data.poll)

getCurrentPollRender: Model -> Maybe PollRender
getCurrentPollRender model =
    (List.head model.slides.future_slides)
        |> Maybe.andThen (\slide -> slide.data.poll_render)


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        GetSlideData read ->
            ( model, read )

        SlideDataRead ( slide_data, slide_contents ) ->
            case Decode.decodeString (Decode.list decodeSlideJSON) slide_data of
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
            ( model
            , Http.post
                { url = "/join"
                , body = Http.stringBody "application/text" model.registration_key
                , expect = Http.expectJson GotWebsocketAddress joinPresentationResponseDecoder
                }
            )

        GotWebsocketAddress response ->
            case response of
                -- We successfully authenticated to the presentation,
                -- open the websocket via the port
                Ok startPresentationResponse ->
                    update (StartPresentation startPresentationResponse.url) { model | state = Authenticated startPresentationResponse }

                -- Should try and do something here to notify the presenter something went wrong
                Err _ ->
                    ( {model | status = Just "Unsuccessful response from POST /join"}, Cmd.none )

        -- Handle the response from the REST API with our websocket address
        -- We need to send a message to the port even before the websocket is
        -- open to force Elm to create it.
        StartPresentation url ->
            ( model, Cmd.batch [ socketConnect url, sendMessage "Hello" ] )

        ReceivedWebsocketMessage message ->
            if model.killed then
                (model, Cmd.none)
            else
                let _ = Debug.log "Message" message in
                case Decode.decodeString receivedWebsocketMessageDecoder message of          
                    Ok (Emoji emoji_msg) -> 
                        (model, addAnimatedEmoji (emoji_msg.emoji, emoji_msg.size))
                    Ok (PollResults poll_results) -> 
                        let _ = Debug.log "Poll Results" poll_results in
                        ( {model | poll_results = poll_results}, Cmd.none)
                    Ok (Error e) -> ({model | status = Just e}, Cmd.none)
                    Err e -> ({model | status = Just (errorToString e)}, Cmd.none)


        -- Reconnect to the presentation if disconnected
        SocketDisconnected _ ->
            -- update AuthenticateToPresentation { model | state = Disconnected, status = Just "Connection to presentation lost, attempting reconnect" }
            update AuthenticateToPresentation model -- Silent reconnect attempt

        NextSlide -> case model.slides.future_slides of
            -- This shouldn't be possible but if it is, we just want to keep the UI the same
            [] -> ( model, Cmd.none )
            -- If this is the last slide, also don't allow moving forward
            _ :: [] -> (model, Cmd.none)
            -- There are still more future slides
            shown_slide :: new_slide :: _ ->
                let 
                    updatedModel = 
                        let
                            updatedSlidesModel = 
                                { model
                                 | slides = { past_slides = shown_slide :: model.slides.past_slides, future_slides = List.drop 1 model.slides.future_slides }
                                 , poll_render = Nothing
                                }
                        in
                            case (new_slide.data.poll, new_slide.data.poll_render) of
                                (Just _, Just poll_render) -> 
                                    { updatedSlidesModel | poll_render = Just poll_render }
                                _ -> 
                                    updatedSlidesModel

                    emojiUpdateCmd = 
                        sendMessage (encodeSlideDataAsNewSlideMessage new_slide.data ((List.length model.slides.past_slides) + 1)) 

                    (pollUpdateCmd, pollIntervalCmd) = 
                        case (new_slide.data.poll, new_slide.data.poll_render) of
                            (Just poll, Just poll_render) -> 
                                -- If there is a poll, we need to do a few things:
                                -- 1. Update the slide emojis as usual
                                -- 2. Update the server with the new poll to start collecting results
                                -- 3. Starting polling the backend with the requested interval to show the results in real time
                                (sendMessage (encodePollAsNewPollMessage poll), delay poll_render.refresh_interval UpdatePollResults)
                            _ ->
                                ( Cmd.none, Cmd.none)

                    addRateLimiterCmd = 
                        case new_slide.data.addRateLimiter of 
                            Just limiter ->
                                sendMessage (encodeAddRateLimiterAsNewRateLimiterMessage limiter)
                            _ -> 
                                Cmd.none

                    removeRateLimiterCmd = 
                        case new_slide.data.removeRateLimiter of 
                            Just limiterName ->
                                sendMessage (encodeRemoveRateLimiterAsRemoveRateLimiterMessage limiterName)
                            _ -> 
                                Cmd.none

                in
                    -- NOTE: It is imperative that pollUpdateCmd comes BEFORE emojiUpdateCmd as the join client purges poll data on new slide data recieved
                    ( updatedModel, Cmd.batch [pollUpdateCmd, pollIntervalCmd, emojiUpdateCmd, addRateLimiterCmd, removeRateLimiterCmd]) 

        PreviousSlide ->
            case List.head model.slides.past_slides of
                Just slide ->
                    ( { model
                        | slides = { past_slides = List.drop 1 model.slides.past_slides, future_slides = slide :: model.slides.future_slides }
                    }
                    , sendMessage (encodeSlideDataAsNewSlideMessage slide.data ((List.length model.slides.past_slides) - 1))
                    )
                Nothing ->
                    ( model, Cmd.none )

        KillSwitch ->
            case model.killswitch_count of
                -- Execute the killswitch and stop showing emojis
                5 ->
                    let _ = Debug.log "KillSwitch" "The killswitch has been activated" in
                    ( { model | killswitch_count = 0 , killed = True}, Cmd.none )
                -- Increment the killswitch count
                _ ->
                    ( { model | killswitch_count = model.killswitch_count + 1 }, Cmd.none )
    
        ToggleAspect -> 
            let 
                flipped = not model.stretchedMode 
            in
                ({model | stretchedMode = flipped}, Cmd.none)

        OtherKey _ ->
            ( model, Cmd.none )

        UpdatePollResults ->
            let _ = Debug.log "Timer Elapsed" () in
            case (getCurrentPoll model, getCurrentPollRender model) of
                (Just poll, Just poll_render) ->
                    ( model, Cmd.batch [
                        delay poll_render.refresh_interval UpdatePollResults
                        , sendMessage (encodePollAsRequestTotalsMessage poll)
                        ])
                _ ->
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
    -- TODO if presentation not started, show input view
    if model.state == Disconnected then 
        -- Render input view for initial key + slides images and settings file select
        div [] 
            [ div [ class "title-group" ]
                [ div [ class "title-gradient-text-container" ] [
                    div [ class "title" ] 
                        [ span [] [text "Presenter View - Start Presentation" ] ]
                    ]
                , div [ class "title-group-divider" ] []
                ]
            , div [ class "input-view" ]
                [ 
                -- Registration key fields
                div [class "reg-key-container"] 
                    [ div [ class "input-group-title" ] [text "Registration Key"]
                    , input [ class "slides-input", type_ "text", value model.registration_key, onInput ChangeRegistrationKey ] []
                    ]
                -- Multi input file select
                , div [ class "input-group-title" ] [text "Slide Files"]
                , div [ class "slides-file-input" ] 
                    [ label [for "slides-file-input"]  [ text "Select slides and JSON settings files" ]
                    , input [ id "slides-file-input", type_ "file", multiple True, on "change" filesDecoderMsg ] []
                ]
                -- Status text
                , div [ class "status-container" ] [
                    case model.status of
                        Just status ->
                            text status
                        Nothing ->
                            text "No slide images or settings JSON selected"
                    ]
                -- Slides preview, and start button when populated
                , div [ class "slides-container-preview" ] [
                    case List.head model.slides.future_slides of
                        Just slide ->
                            div [] [
                                img [ class "slide-img", src slide.image] []
                                , div [ class "start-presentation-button", onClick AuthenticateToPresentation ] 
                                    [ text "Start Presentation" ]
                            ]
                        Nothing -> div [] []
                ]
                ]
        ]
    else 
        -- Render presentation view, slides img container and polls

        div [ class "slides-container", classList [("stretched", model.stretchedMode == True)] ] [
            case List.head model.slides.future_slides of
                Just slide ->
                    img [ class "slide-img", src slide.image] []
                Nothing -> div [] []
            , div [ id "poll-results-container" ]
            [ 
                case model.poll_render of
                    (Just render) ->
                        div [ 
                            id "poll-results"
                        ,   style "left" (String.fromInt render.x ++ "%")
                        ,   style "top" (String.fromInt render.y ++ "%")
                        ,   style "width" (String.fromInt render.scale ++ "%")
                        ]
                        [ Exhibit.Visualizations.Centroid.view (List.map (\(f, s) -> (f, Basics.toFloat s)) (Dict.toList model.poll_results)) 500 500 render.vbx render.vby ]
                    _ -> div [ id "poll-results-container" ] []
            ]
            , div [ id "reactions-float-bottom" ]
                [ div [ id "reactions-container" ] []
                ]
        ]