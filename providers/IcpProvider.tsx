"use client";

import React, {
    createContext,
    ReactNode,
    useContext,
    useMemo,
    useState,
} from "react";
import {Actor, HttpAgent} from "@dfinity/agent";
import {idlFactory} from "@/candid/service.did.js";

const CANISTER_ID = "qhsyi-dyaaa-aaaai-q3s4a-cai";
const ICP_HOST = "https://icp0.io";
// const CANISTER_ID = "by6od-j4aaa-aaaaa-qaadq-cai";
// const ICP_HOST = "http://127.0.0.1:4943";

export interface CrawlResult {
    prediction_id: string;
    web_entities: string[];
    full_matching_images: string[];
    pages_with_matching_images: string[];
    visually_similar_images: string[];
    last_update: number;
}

export interface CrawledResults {
    [imageName: string]: CrawlResult;
}

interface IcpContextType {
    isCrawling: (predictionId: string) => boolean;
    crawlingIds: string[];
    crawledResults: CrawledResults;
    loadStoredResults: (userId: string) => Promise<void>;
    getImagesNamesById: (userId: string) => Promise<string[]>;
    fetchAllImagesById: (
        userId: string
    ) => Promise<{ name: string; url: string }[]>;
    crawlImage: (
        userId: string,
        predictionId: string,
        imageName: string,
        imageContent: number[]
    ) => Promise<CrawlResult>;
    crawlImageNotStored: (
        userId: string,
        predictionId: string,
        imageName: string,
        imageContent: number[]
    ) => Promise<CrawlResult>;
}

const IcpContext = createContext<IcpContextType | undefined>(undefined);

export const IcpProvider: React.FC<{ children: ReactNode }> = ({
                                                                   children,
                                                               }) => {
    const agent = new HttpAgent({host: ICP_HOST});
    const [crawlingIds, setCrawlingIds] = useState<string[]>([]);
    const [crawledResults, setCrawledResults] = useState<CrawledResults>({});

    if (process.env.NEXT_PUBLIC_DFX_NETWORK !== "ic") {
        agent.fetchRootKey().catch((err) => {
            console.error("Unable to fetch root key:", err);
        });
    }

    const backendActor = Actor.createActor(idlFactory, {
        agent,
        canisterId: CANISTER_ID,
    });

    const isCrawling = (predictionId: string): boolean =>
        crawlingIds.includes(predictionId);

    const getImagesNamesById = async (userId: string): Promise<string[]> => {
        try {
            const imageNames: string[] = (await backendActor.list_images(
                userId
            )) as string[];
            return imageNames;
        } catch (error) {
            console.error("Error fetching image names:", error);
            return [];
        }
    };

    const fetchAllImagesById = async (
        userId: string
    ): Promise<{ name: string; url: string }[]> => {
        try {
            const imageNames = await getImagesNamesById(userId);

            const imageDetails = await Promise.all(
                imageNames.map(async (name) => {
                    const imageResult = (await backendActor.get_image(userId, name)) as {
                        Ok?: { content: number[] };
                        Err?: string;
                    };

                    if (imageResult.Ok?.content) {
                        const storedImage = imageResult.Ok;
                        const blob = new Blob([new Uint8Array(storedImage.content)], {
                            type: "image/jpeg",
                        });

                        return new Promise<{ name: string; url: string }>(
                            (resolve, reject) => {
                                const reader = new FileReader();
                                reader.onloadend = () => {
                                    if (reader.result) {
                                        resolve({name, url: reader.result.toString()});
                                    } else {
                                        reject("Failed to read the Blob as Data URL");
                                    }
                                };
                                reader.onerror = (err) => reject(err);
                                reader.readAsDataURL(blob);
                            }
                        );
                    } else {
                        console.error(`Error fetching image '${name}':`, imageResult.Err);
                        return null;
                    }
                })
            );

            return imageDetails.filter(Boolean) as Array<{
                name: string;
                url: string;
            }>;
        } catch (error) {
            console.error("Error fetching all images:", error);
            return [];
        }
    };

    const loadStoredResults = async (userId: string): Promise<void> => {
        try {
            const storedResults = (await backendActor.get_crawl_results(userId)) as {
                Ok?: string;
                Err?: string;
            };

            if (storedResults.Ok) {
                const parsedStoredResults = JSON.parse(storedResults.Ok);
                setCrawledResults(parsedStoredResults);
            } else {
                console.error("Error loading stored results:", storedResults.Err);
            }
        } catch (error) {
            console.error("Error loading stored results:", error);
        }
    };

    const crawlImage = async (
        userId: string,
        predictionId: string,
        imageName: string,
        imageContent: number[]
    ): Promise<CrawlResult> => {
        setCrawlingIds((prev) => [...prev, predictionId]);
        try {
            const imageNames = await getImagesNamesById(userId);

            if (!imageNames.includes(imageName)) {
                const storeResult = (await backendActor.store_image(
                    userId,
                    predictionId,
                    imageName,
                    imageContent
                )) as { Ok?: string; Err?: string };

                if (storeResult.Err) {
                    throw new Error(`Failed to store image: ${storeResult.Err}`);
                }
            }

            const detectionResult = (await backendActor.detect_image(
                userId,
                predictionId,
                imageName
            )) as { Ok?: string; Err?: string };

            if (detectionResult.Ok) {
                const parsedResult = JSON.parse(detectionResult.Ok);
                setCrawledResults((prevResults) => ({
                    ...prevResults,
                    [imageName]: parsedResult,
                }));
                return parsedResult;
            } else {
                throw new Error(`Failed to detect image: ${detectionResult.Err}`);
            }
        } catch (error) {
            console.error("Error crawling image:", error);
            throw error;
        } finally {
            setCrawlingIds((prev) => prev.filter((id) => id !== predictionId));
        }
    };

    const crawlImageNotStored = async (
        userId: string,
        predictionId: string,
        imageName: string,
        imageContent: number[]
    ): Promise<CrawlResult> => {
        setCrawlingIds((prev) => [...prev, predictionId]);
        try {
            const detectionResult = (await backendActor.detect_image_with_content(
                userId,
                predictionId,
                imageName,
                imageContent
            )) as { Ok?: string; Err?: string };

            if (detectionResult.Ok) {
                const parsedResult = JSON.parse(detectionResult.Ok);
                setCrawledResults((prevResults) => ({
                    ...prevResults,
                    [imageName]: parsedResult,
                }));
                return parsedResult;
            } else {
                throw new Error(`Failed to detect image: ${detectionResult.Err}`);
            }
        } catch (error) {
            console.error("Error crawling image:", error);
            throw error;
        } finally {
            setCrawlingIds((prev) => prev.filter((id) => id !== predictionId));
        }
    };

    const contextValue = useMemo(
        () => ({
            isCrawling,
            crawlingIds,
            crawledResults,
            loadStoredResults,
            getImagesNamesById,
            fetchAllImagesById,
            crawlImage,
            crawlImageNotStored,
        }),
        [
            crawlingIds,
            crawledResults,
            getImagesNamesById,
            fetchAllImagesById,
            crawlImage,
            crawlImageNotStored,
        ]
    );

    return (
        <IcpContext.Provider value={contextValue}>{children}</IcpContext.Provider>
    );
};

export const useIcp = () => {
    const context = useContext(IcpContext);
    if (context === undefined) {
        throw new Error("useIcp must be used within an IcpProvider");
    }
    return context;
};
