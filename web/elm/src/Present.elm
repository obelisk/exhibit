port module Present exposing (..)

import Browser
import Browser.Events
import Dict exposing (Dict)
import Exhibit.IO exposing (..)
import Exhibit.Utils exposing (getAtIndex, popLast)
import File exposing (..)
import Html exposing (Html, button, div, img, input, label, text, span)
import Html.Attributes exposing (class, for, id, multiple, type_, value, classList, style)
import Html.Events exposing (on, onClick, onInput)
import Http
import Json.Decode as Decode exposing (field, string, int)
import Task exposing (..)
import Html.Attributes exposing (src)
import Json.Encode
import Exhibit.ServerMessagePresenterTypes exposing (receivedWebsocketMessageDecoder, ReceivedMessage(..))
import Json.Decode exposing (errorToString)
import Exhibit.UserMessageTypes exposing (encodeVoteType)
import Process



-- Ports


port socketConnect : String -> Cmd msg


port sendMessage : String -> Cmd msg


port addAnimatedEmoji : ( String, Int ) -> Cmd msg


port messageReceived : (String -> msg) -> Sub msg


port socketDisconnected : (String -> msg) -> Sub msg

port toggleFullscreen : () -> Cmd msg


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
    | ToggleFullscreen
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
    { refreshInterval : Int
    , x : Int
    , y : Int
    , scale : Float
    }

pollRenderDecoder : Decode.Decoder PollRender
pollRenderDecoder = 
    Decode.map4 PollRender
        (field "refreshInterval" Decode.int)
        (field "x" Decode.int)
        (field "y" Decode.int)
        (field "scale" Decode.float)

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

rateLimiterOptionsDecoder : Decode.Decoder RateLimiterOptions
rateLimiterOptionsDecoder =
    Decode.map2 RateLimiterOptions
        (Decode.maybe (field "addRateLimiter" Decode.value))
        (Decode.maybe (field "removeRateLimiter" Decode.string))

proceedFromPollResultOptsDecoder : Decode.Decoder ProceedFromPollResultOpts
proceedFromPollResultOptsDecoder =
    Decode.map2 ProceedFromPollResultOpts
        (field "poll_name" string)
        (field "results_to_slide_number" (Decode.list int))

decodeSlideJSON : Decode.Decoder SlideData
decodeSlideJSON =
    Decode.map8 SlideData
        (Decode.maybe (field "poll" pollDecoder))
        (Decode.maybe (field "poll_render" pollRenderDecoder))
        (Decode.maybe (field "rateLimiterOptions" rateLimiterOptionsDecoder))
        (field "slide" string)
        (field "message" string)
        (field "emojis" (Decode.list string))
        (Decode.maybe (field "next_slide_index" int))
        (Decode.maybe (field "slide_advancement_from_poll_results" proceedFromPollResultOptsDecoder))

type alias SlideData =
    { poll : Maybe Poll
    , currentPollRender : Maybe PollRender 
    , rateLimiterOptions : Maybe RateLimiterOptions
    , slide : String
    , message : String
    , emojis : List String
    , nextSlideIndex : Maybe Int
    , nextSlideFromPollOptions : Maybe ProceedFromPollResultOpts
    }

type alias RateLimiterOptions = 
    { addRateLimiter : Maybe Decode.Value
    , removeRateLimiter : Maybe String
    }

type alias ProceedFromPollResultOpts = 
    { pollName : String
    , pollChoicesToSlideIndices : List Int
    }

type alias Slide =
    { data : SlideData
    , image : String 
    }

type alias Model =
    { registration_key : String
    , status : Maybe String
    , allSlides : List Slide
    , currentSlide : Maybe Slide
    , resolvedSlideHistory: List Int
    , currentSlideIndex : Int
    , state : State
    , currentPollResults : Dict String Int
    , currentPollRender : Maybe PollRender
    , allPollResults : Dict String Int -- Lookup of poll name to index of winning option
    , killswitch_count : Int
    , killed : Bool
    , stretchedMode : Bool
    }


-- Keyboard controls decoder and message
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

        "f" ->
            ToggleFullscreen

        x ->
            OtherKey x


init : Maybe String -> ( Model, Cmd Msg )
init registration_key =
    ( { registration_key = Maybe.withDefault "" registration_key
      , status = Nothing
      , allSlides = []
      , currentSlide = Nothing
      , resolvedSlideHistory = []
      , state = Disconnected
      , currentPollResults = Dict.empty
      , currentPollRender = Nothing
      , allPollResults = Dict.empty
      , currentSlideIndex = 0
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
                            , allSlides = slides
                            , currentSlide = getAtIndex slides 0
                            , resolvedSlideHistory = []
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
                -- let _ = Debug.log "Message" message in
                case Decode.decodeString receivedWebsocketMessageDecoder message of          
                    Ok (Emoji emoji_msg) -> 
                        (model, addAnimatedEmoji (emoji_msg.emoji, emoji_msg.size))
                    Ok (PollResults currentPollResults) -> 
                        -- let _ = Debug.log "Poll Results" currentPollResults in
                        ( {model | currentPollResults = currentPollResults}, Cmd.none)
                    Ok (Error e) -> ({model | status = Just e}, Cmd.none)
                    Err e -> ({model | status = Just (errorToString e)}, Cmd.none)


        -- Reconnect to the presentation if disconnected
        SocketDisconnected _ ->
            -- update AuthenticateToPresentation { model | state = Disconnected, status = Just "Connection to presentation lost, attempting reconnect" }
            update AuthenticateToPresentation model -- Silent reconnect attempt

        NextSlide -> 
            case model.currentSlide of 
                Just currentSlide ->
                    computeNextSlide model currentSlide
                Nothing ->
                    ( model, Cmd.none)
            

        PreviousSlide ->
            -- We keep a running history of the resolved slide indices
            -- So just pop the list and go to that slide
            let
                (poppedHistoryIndex, poppedHistoryList) = 
                    popLast model.resolvedSlideHistory
                        |> Maybe.withDefault (0, [])

                newSlide = 
                    getAtIndex model.allSlides poppedHistoryIndex
                
                updatedModel = 
                    { model
                        | currentSlide = newSlide 
                        , currentPollRender = Nothing
                        , resolvedSlideHistory = poppedHistoryList
                    }

                _ = Debug.log "poppedHistoryIndex is " poppedHistoryIndex              
                _ = Debug.log "poppedHistoryList is " poppedHistoryList    

                _ = 
                    case newSlide of 
                        Just s -> 
                            let _ = Debug.log "popped newSlide data is "s.data in
                            Nothing
                        Nothing -> 
                            let _ = Debug.log "next slide is none!! " in
                            Nothing          
            in
                case newSlide of
                    Just s -> 
                        ( updatedModel
                        , sendMessage (encodeSlideDataAsNewSlideMessage s.data poppedHistoryIndex) )
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
    
        ToggleFullscreen -> 
            (model, toggleFullscreen ())

        OtherKey _ ->
            ( model, Cmd.none )

        UpdatePollResults ->
            let _ = Debug.log "Timer Elapsed" () in
            case model.currentSlide of 
                Just currentSlide ->
                    case (currentSlide.data.poll, currentSlide.data.currentPollRender) of
                        (Just poll, Just currentPollRender) ->
                            ( model, Cmd.batch [
                                delay currentPollRender.refreshInterval UpdatePollResults
                                , sendMessage (encodePollAsRequestTotalsMessage poll)
                                ])
                        _ ->
                            ( model, Cmd.none )
                Nothing ->
                    ( model, Cmd.none )

computeNextSlide : Model -> Slide -> (Model, Cmd Msg)
computeNextSlide model currentSlide = 
    let
        -- From the server we get a list of (pollLabel, count) as model.currentPollResults
        --
        -- We know the current slide's poll options as list
        -- ex.  Current poll:               [ "A", "B", "C" ]
        --      Votes from server:          [ ("B", 3), ("C", 9), ("A", 5) ]
        --      Poll Advancement config:    [ 33, 50, 90]
        --                                             ^
        --
        -- Since "C" is the winning pollOption with 9 votes, allPollResults[currentSlidePollName] should
        -- be set to 2, representing the winning index ("C")
        -- This function returns a tuple of (winningPollOptionLabel, winningOptionIndex) 
        -- where winningOptionIndex is later used to look up which slide to advance to via the Poll Advancement config list (indices match)
        highestVotedOptionIndexOfCurrentSlidePoll : (Int, String)
        highestVotedOptionIndexOfCurrentSlidePoll = 
            let
                maxTupleValue (k1, v1) (k2, v2) =
                    if v1 > v2 then
                        (k1, v1)
                    else
                        (k2, v2)

                -- Give a Poll and the winning tuple, return the index of the winning option
                findWinningPollOptionIndex : Poll -> Dict String Int -> Maybe (Int, String)
                findWinningPollOptionIndex currentPoll currentPollResults = 
                    Dict.toList currentPollResults
                        |> List.foldl maxTupleValue ( Maybe.withDefault "" (getAtIndex currentPoll.options 0), 0)
                        |> (\(winningOptionLabel, _) -> 
                            let
                                indexedPollOptionsTuple = 
                                    List.indexedMap Tuple.pair currentPoll.options
                            in
                                indexedPollOptionsTuple
                                    |> List.filter (\(_, label) -> label == winningOptionLabel) 
                        ) 
                        |> List.head
            in
                currentSlide.data.poll
                    |> Maybe.andThen (\currentPoll -> findWinningPollOptionIndex currentPoll model.currentPollResults) 
                    |> Maybe.withDefault (0, "")

        -- _ = Debug.log "highestVotedOptionIndexOfCurrentSlidePoll is " highestVotedOptionIndexOfCurrentSlidePoll
        
        -- Set up possible new state to top level seen polls, this will be used to set new 
        -- top level poll state in model, but also allows current slide to reference 
        -- the current slide's poll results the same way
        newAllPollResults : Dict String Int
        newAllPollResults = 
            case currentSlide.data.poll of 
                Just poll -> 
                    Dict.insert poll.name (Tuple.first highestVotedOptionIndexOfCurrentSlidePoll) model.allPollResults
                Nothing -> 
                    model.allPollResults

        _ = Debug.log "newAllPollResults is " newAllPollResults
        
        -- Begin logic to apply the following rules to determine what the next slide is
        (nextSlide, nextSlideIndex) = 
            case (currentSlide.data.nextSlideIndex, currentSlide.data.nextSlideFromPollOptions) of 
                -- linear, this slide links directly to next by index (nextSlideIndex populated)
                (Just newSlideIndex, Nothing) -> 
                    let _ = Debug.log "linear - next slide is" newSlideIndex in
                    (getAtIndex model.allSlides newSlideIndex, newSlideIndex)

                -- branching, next slide depends on current slide or previously seen poll results
                -- at this point we have already computed current slide Maybe poll added to 
                -- top level allPollResults in local variable newAllPollResults
                (Nothing, Just nextSlideFromPollOptions) -> 
                    case Dict.get nextSlideFromPollOptions.pollName newAllPollResults of 
                        Just previousPollWinningChoiceIndex -> 
                            case (getAtIndex nextSlideFromPollOptions.pollChoicesToSlideIndices previousPollWinningChoiceIndex) of
                                Just winningOptionResolvedNextSlideIndex ->                           
                                    (getAtIndex model.allSlides winningOptionResolvedNextSlideIndex, winningOptionResolvedNextSlideIndex)
                                Nothing -> 
                                    ( Just currentSlide, model.currentSlideIndex )

                        -- No poll with this name found
                        Nothing -> 
                            let _ = Debug.log "No poll with name found in previous poll or current slides poll" nextSlideFromPollOptions.pollName in
                            ( Just currentSlide, model.currentSlideIndex )
                _ -> 
                    ( Just currentSlide, model.currentSlideIndex )

        _ = 
            case nextSlide of 
                Just s -> 
                    let _ = Debug.log "nextSlide data is "s.data in
                    Nothing
                Nothing -> 
                    let _ = Debug.log "next slide is none!! " in
                    Nothing

        maybeNewSlidePollRender : Maybe PollRender
        maybeNewSlidePollRender =
            nextSlide
                |> Maybe.map (\slide -> slide.data)
                |> Maybe.andThen (\data -> data.currentPollRender)

        -- _ = Debug.log "maybeNewSlidePollRender is " maybeNewSlidePollRender

        (updatedSlideHistoryIndices, _) = 
            if nextSlide /= Nothing then
                (List.append model.resolvedSlideHistory [model.currentSlideIndex], -100) -- TODO
            else 
                (model.resolvedSlideHistory, -100)

        -- Create model updates 
        updatedModel = 
            { model
                | currentSlide = nextSlide 
                , currentPollResults = Dict.empty
                , currentPollRender = maybeNewSlidePollRender
                , allPollResults = newAllPollResults
                , resolvedSlideHistory = updatedSlideHistoryIndices
                , currentSlideIndex = nextSlideIndex
            }

        _ = Debug.log "updatedSlideHistoryIndices is " updatedSlideHistoryIndices              
        
    in
        case nextSlide of 
            -- Create possible CMD updates, skip if last slide
            Just newSlide ->
                let
                    emojiUpdateCmd = 
                        sendMessage (encodeSlideDataAsNewSlideMessage newSlide.data nextSlideIndex) 

                    (pollUpdateCmd, pollIntervalCmd) = 
                        case (newSlide.data.poll, newSlide.data.currentPollRender) of
                            (Just poll, Just currentPollRender) -> 
                                -- If there is a poll, we need to do a few things:
                                -- 1. Update the slide emojis as usual
                                -- 2. Update the server with the new poll to start collecting results
                                -- 3. Starting polling the backend with the requested interval to show the results in real time
                                (sendMessage (encodePollAsNewPollMessage poll), delay currentPollRender.refreshInterval UpdatePollResults)
                            _ ->
                                ( Cmd.none, Cmd.none)

                    addRateLimiterCmd = 
                        case newSlide.data.rateLimiterOptions of 
                            Just options -> 
                                case options.addRateLimiter of 
                                    Just limiter ->
                                        sendMessage (encodeAddRateLimiterAsNewRateLimiterMessage limiter)
                                    _ -> 
                                        Cmd.none
                            _ -> 
                                Cmd.none

                    removeRateLimiterCmd = 
                        case newSlide.data.rateLimiterOptions of 
                            Just options -> 
                                case options.removeRateLimiter of 
                                    Just limiterName ->
                                        sendMessage (encodeRemoveRateLimiterAsRemoveRateLimiterMessage limiterName)
                                    _ -> 
                                        Cmd.none
                            _ -> 
                                Cmd.none

                in
                    -- NOTE: It is imperative that pollUpdateCmd comes BEFORE emojiUpdateCmd as the join client purges poll data on new slide data recieved
                    ( updatedModel, Cmd.batch [pollUpdateCmd, pollIntervalCmd, emojiUpdateCmd, addRateLimiterCmd, removeRateLimiterCmd]) 
            Nothing -> 
                ( updatedModel, Cmd.none)


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
    if model.state == Disconnected then 
        -- Render input view for initial key + slides images and settings file select
        div [] 
            [ case model.currentPollRender of
                    (Just render) ->
                        renderBarGraph model.currentPollResults render
                    _ -> div [ class "poll-results-container" ] []
            , div [ class "title-group" ]
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
                    case model.currentSlide of
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
            case model.currentSlide of
                Just slide ->
                    img [ class "slide-img", src slide.image] []
                Nothing -> div [] []
            , case model.currentPollRender of
                (Just render) ->
                    renderBarGraph model.currentPollResults render
                _ -> div [ class "poll-results-container" ] []
            , div [ id "reactions-float-bottom" ]
                [ div [ id "reactions-container" ] []
                ]
        ]



renderBarGraph : Dict String Int -> PollRender -> Html Msg
renderBarGraph currentPollResults render = 
    let
        sortedOptions : List (Int, String, Int)
        sortedOptions = 
            currentPollResults
                |> Dict.toList
                |> List.indexedMap (\index (key, val) -> (index, key, val) )
                |> List.sortBy (\(_, _, val) -> val) 
                |> List.reverse

        (_, _, topOptionCount) = 
            let
                max (indexA, labelA, countA) (indexB, labelB, countB) = 
                    if countA > countB then 
                        (indexA, labelA, countA)
                    else 
                        (indexB, labelB, countB)
                
                empty =
                    (0, "", 1)
            in
                List.foldl max empty sortedOptions

        renderPollResultRow : (Int, String, Int) -> Html Msg
        renderPollResultRow (optionIndex, label, count) = 
            let
                leadingVotesCount = topOptionCount

                countPercentageOfTotal = 
                    (toFloat count) / (toFloat leadingVotesCount)
                        |> (*) 100
            in
                div [ class "poll-result-row", id <| "poll-item-" ++ String.fromInt optionIndex] 
                    [ div [ class "poll-result-label-container" ] 
                        [ div [ class "poll-result-label" ] 
                            [ text label ]
                        , div [ class "poll-result-label-count" ] 
                            [ text <| "(" ++ String.fromInt count ++ ")" ]
                        ]
                    , div [ class <| "poll-result-bar-container poll-result-colouring-" ++ String.fromInt optionIndex ] 
                        [ div [ class "poll-result-bar", style "width" <| "" ++ String.fromFloat countPercentageOfTotal ++ "%"] 
                            [] 
                        ]
                    ]
    in
        -- Top level absoulute positioning container
        div [ class "poll-results-container" 
            , style "left" (String.fromInt render.x ++ "%")
            , style "top" (String.fromInt render.y ++ "%") ] 
            [ div [ class "poll-results-scale", style "transform" ("scale(" ++ String.fromFloat render.scale ++ " )" ) ] 
            [ div [ class "poll-vote-now"] [text "- VOTE NOW -"]
            , div [ class "poll-results-text"] [text "Poll results"]
            -- Poll contents sizing and scale container
            , div [ class "poll-results", classList [ ("shrink-height", (List.length sortedOptions) > 4)] ]
                    [  
                        -- Container of result item for label, vote count, and visual coloured bar
                        div [] (List.map renderPollResultRow sortedOptions)
                    ]
                ]
            ]
